//! The part feature tree: an ordered list of solid features (sketches are not
//! features) with per-feature suppression and a part-level rollback bar.
//!
//! Order is the order of feature nodes in `data.cad.nodes` (mirrored in
//! `data.tree.orderedNodeIds` and `data.cad.rootNodeIds`). Suppression is
//! `"suppressed": true` on the graph node and the rollback bar is
//! `data.tree.rollbackIndex` (number of features before the bar). Both are
//! native extensions the web loader ignores.
//!
//! Evaluation walks the features in order up to the bar, skipping suppressed
//! ones. A feature whose inputs are missing (sketch or profile gone, target
//! body suppressed or failed, face source inactive) or whose Boolean fails is
//! marked `Error` and skipped instead of failing the whole part. Results are
//! memoized per feature: a feature is rebuilt only when it, or anything
//! before it, changed.

use std::cell::{Cell, RefCell};

use super::*;

/// The kinds of graph nodes that are features. New kinds (revolve, loft,
/// fillet, ...) slot in here and in `ProjectFile::evaluate_feature`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Extrude,
}

impl FeatureKind {
    pub fn from_node(node: &Value) -> Option<Self> {
        match node.get("type")?.as_str()? {
            "extrude" => Some(Self::Extrude),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeatureStatus {
    Active,
    Suppressed,
    /// After the rollback bar.
    RolledBack,
    Error(String),
}

impl FeatureStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Body the next cut/join applies to: the id of the nearest earlier new-body
/// feature and its index in the evaluated bodies (None when it is inactive).
type BodyTarget = Option<(String, Option<usize>)>;

#[derive(Clone)]
struct CacheEntry {
    key: String,
    status: FeatureStatus,
    preview: SolidPreview,
    bodies: Vec<Manifold>,
    body_ids: Vec<String>,
    target: BodyTarget,
}

/// Memoized per-feature evaluation; see the module docs.
#[derive(Default)]
pub(super) struct FeatureCache {
    entries: RefCell<Vec<CacheEntry>>,
    evaluations: Cell<usize>,
}

pub(super) struct TreeEvaluation {
    pub previews: Vec<SolidPreview>,
    pub statuses: Vec<FeatureStatus>,
    pub bodies: Vec<Manifold>,
}

fn placeholder(node: &Value) -> SolidPreview {
    SolidPreview {
        name: node
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Feature")
            .to_string(),
        profile: Vec::new(),
        holes: Vec::new(),
        depth: node.get("depth").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        frame: SketchPlane::Top.frame(),
        start: 0.0,
    }
}

fn preview_key(preview: &SolidPreview) -> String {
    format!(
        "{:?}|{:?}|{:?}|{}",
        preview.profile, preview.holes, preview.frame, preview.depth
    )
}

impl ProjectFile {
    pub(super) fn feature_nodes(&self) -> Vec<&Value> {
        self.graph_nodes()
            .map(|nodes| {
                nodes
                    .iter()
                    .filter(|node| FeatureKind::from_node(node).is_some())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn feature_count(&self) -> usize {
        self.feature_nodes().len()
    }

    /// Rollback bar position: the number of features evaluated, or None
    /// when the bar is at the end.
    pub fn rollback_index(&self) -> Option<usize> {
        let count = self.feature_count();
        self.value
            .pointer(&format!(
                "/items/{}/data/tree/rollbackIndex",
                self.part_index
            ))
            .and_then(Value::as_u64)
            .map(|index| index as usize)
            .filter(|&index| index < count)
    }

    pub fn set_rollback_index(&mut self, index: Option<usize>) -> Result<(), String> {
        let count = self.feature_count();
        let tree = self
            .value
            .pointer_mut(&format!("/items/{}/data/tree", self.part_index))
            .and_then(Value::as_object_mut)
            .ok_or("project tree is invalid")?;
        match index.filter(|&index| index < count) {
            Some(index) => {
                tree.insert("rollbackIndex".into(), json!(index));
            }
            None => {
                tree.remove("rollbackIndex");
            }
        }
        Ok(())
    }

    pub fn feature_suppressed(&self, index: usize) -> bool {
        self.feature_nodes()
            .get(index)
            .is_some_and(|node| node.get("suppressed").and_then(Value::as_bool) == Some(true))
    }

    pub fn set_feature_suppressed(&mut self, index: usize, suppressed: bool) -> Result<(), String> {
        let node = self
            .feature_node_mut(index)
            .ok_or("feature node is missing")?;
        let object = node.as_object_mut().ok_or("feature node is invalid")?;
        if suppressed {
            object.insert("suppressed".into(), json!(true));
        } else {
            object.remove("suppressed");
        }
        Ok(())
    }

    fn feature_node_mut(&mut self, index: usize) -> Option<&mut Value> {
        self.value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))?
            .as_array_mut()?
            .iter_mut()
            .filter(|node| FeatureKind::from_node(node).is_some())
            .nth(index)
    }

    /// Status of every feature, in order.
    pub fn feature_statuses(&self) -> Vec<FeatureStatus> {
        self.evaluate_tree()
            .map(|tree| tree.statuses)
            .unwrap_or_default()
    }

    /// Previews of the features that contribute to the evaluated part.
    pub fn active_extrusion_previews(&self) -> Option<Vec<SolidPreview>> {
        let tree = self.evaluate_tree()?;
        Some(
            tree.previews
                .into_iter()
                .zip(tree.statuses)
                .filter(|(_, status)| status.is_active())
                .map(|(preview, _)| preview)
                .collect(),
        )
    }

    /// Number of active features in error; None when the tree does not
    /// evaluate at all.
    pub(super) fn feature_error_count(&self) -> Option<usize> {
        Some(
            self.evaluate_tree()?
                .statuses
                .iter()
                .filter(|status| matches!(status, FeatureStatus::Error(_)))
                .count(),
        )
    }

    /// True when the tree evaluates with no more errors than `before`.
    pub(super) fn no_new_errors(&self, before: Option<usize>) -> bool {
        self.feature_error_count()
            .is_some_and(|after| after <= before.unwrap_or(0))
    }

    /// How many features have been (re)built since the project was opened.
    pub fn feature_evaluations(&self) -> usize {
        self.feature_cache.evaluations.get()
    }

    /// Ids a feature must come after, given the feature order `nodes`:
    /// explicit references plus the body a cut/join implicitly applies to.
    fn feature_dependencies(&self, nodes: &[&Value], index: usize) -> Vec<String> {
        let node = nodes[index];
        let mut deps = Vec::new();
        if matches!(node["operation"].as_str(), Some("cut" | "join"))
            && let Some(body) = nodes[..index]
                .iter()
                .rev()
                .find(|node| node["operation"] == "newBody")
                .and_then(|node| node["id"].as_str())
        {
            deps.push(body.to_string());
        }
        if let Ok(ExtrudeOptions {
            end: EndCondition::UpToBody(target),
            ..
        }) = extrude_options(node)
        {
            deps.push(target);
        }
        if let Some(face) = node["sketchId"]
            .as_str()
            .and_then(|id| self.sketches.iter().find(|sketch| sketch.id == id))
            .and_then(|sketch| sketch.face.as_ref())
        {
            deps.push(face.extrude_id.clone());
        }
        for key in ["sourceId", "targetId"] {
            if let Some(id) = node.get(key).and_then(Value::as_str) {
                deps.push(id.to_string());
            }
        }
        deps
    }

    /// Move feature `index` by `delta` places. Refused when a feature would
    /// come before something it depends on, or a cut/join would change the
    /// body it applies to. Returns the new index.
    pub fn move_feature(&mut self, index: usize, delta: isize) -> Result<usize, String> {
        let nodes = self.feature_nodes();
        let target = index as isize + delta;
        if index >= nodes.len() || target < 0 || target as usize >= nodes.len() {
            return Err("feature cannot move further".into());
        }
        let target = target as usize;
        let mut order: Vec<&Value> = nodes.clone();
        let moved = order.remove(index);
        order.insert(target, moved);
        for (position, node) in order.iter().enumerate() {
            let before: Vec<&str> = order[..position]
                .iter()
                .filter_map(|node| node["id"].as_str())
                .collect();
            let old_position = nodes
                .iter()
                .position(|candidate| std::ptr::eq(*candidate, *node))
                .expect("same nodes");
            let old_deps = self.feature_dependencies(&nodes, old_position);
            let new_deps = self.feature_dependencies(&order, position);
            let name = node["name"].as_str().unwrap_or("feature");
            if let Some(missing) = new_deps.iter().find(|id| !before.contains(&id.as_str())) {
                return Err(format!("{name} depends on {missing}"));
            }
            if old_deps.first() != new_deps.first()
                && matches!(node["operation"].as_str(), Some("cut" | "join"))
            {
                return Err(format!("{name} would apply to a different body"));
            }
        }
        let ids: Vec<String> = order
            .iter()
            .filter_map(|node| node["id"].as_str().map(str::to_string))
            .collect();
        let errors = self.feature_error_count();
        let snapshot = self.feature_tree_snapshot();
        self.apply_feature_order(&ids)?;
        if !self.no_new_errors(errors) {
            self.restore_feature_tree(snapshot)?;
            return Err("the reordered features would not evaluate".into());
        }
        Ok(target)
    }

    fn apply_feature_order(&mut self, ids: &[String]) -> Result<(), String> {
        let data = self
            .value
            .pointer_mut(&format!("/items/{}/data", self.part_index))
            .ok_or("part data is missing")?;
        let nodes = data["cad"]["nodes"]
            .as_array_mut()
            .ok_or("project CAD graph is invalid")?;
        let slots: Vec<usize> = (0..nodes.len())
            .filter(|&i| FeatureKind::from_node(&nodes[i]).is_some())
            .collect();
        let mut taken: Vec<Value> = slots.iter().map(|&i| nodes[i].clone()).collect();
        for (slot, id) in slots.iter().zip(ids) {
            let position = taken
                .iter()
                .position(|node| node["id"].as_str() == Some(id))
                .ok_or("feature order is inconsistent")?;
            nodes[*slot] = taken.remove(position);
        }
        for path in ["/cad/rootNodeIds", "/tree/orderedNodeIds"] {
            let Some(list) = data.pointer_mut(path).and_then(Value::as_array_mut) else {
                continue;
            };
            let slots: Vec<usize> = (0..list.len())
                .filter(|&i| {
                    list[i]
                        .as_str()
                        .is_some_and(|entry| ids.iter().any(|id| id == entry))
                })
                .collect();
            let present: Vec<&String> = ids
                .iter()
                .filter(|id| slots.iter().any(|&i| list[i].as_str() == Some(id.as_str())))
                .collect();
            for (slot, id) in slots.iter().zip(present) {
                list[*slot] = json!(id);
            }
        }
        Ok(())
    }

    /// Graph nodes, roots and tree of the active part, for undo.
    pub fn feature_tree_snapshot(&self) -> Value {
        let data = &self.value["items"][self.part_index]["data"];
        json!({
            "nodes": data["cad"]["nodes"].clone(),
            "roots": data["cad"]["rootNodeIds"].clone(),
            "tree": data["tree"].clone(),
        })
    }

    pub fn restore_feature_tree(&mut self, snapshot: Value) -> Result<(), String> {
        let data = self
            .value
            .pointer_mut(&format!("/items/{}/data", self.part_index))
            .ok_or("part data is missing")?;
        if !snapshot["nodes"].is_array() {
            return Err("feature snapshot is invalid".into());
        }
        data["cad"]["nodes"] = snapshot["nodes"].clone();
        data["cad"]["rootNodeIds"] = snapshot["roots"].clone();
        data["tree"] = snapshot["tree"].clone();
        Ok(())
    }

    /// Copy the open sketch into its workspace so the next evaluation sees
    /// edits made since the last switch (re-evaluates dependent features).
    pub fn sync_active_document(&mut self, current: &Document) {
        self.sketches[self.active_sketch].document = current.clone();
    }

    /// Evaluate every feature in order. None when the part has no features
    /// or contains nodes the native evaluator does not support.
    pub(super) fn evaluate_tree(&self) -> Option<TreeEvaluation> {
        let nodes = self.graph_nodes()?;
        if nodes
            .iter()
            .any(|node| node.get("type").and_then(Value::as_str) == Some("chamfer"))
        {
            return None;
        }
        let features = self.feature_nodes();
        if features.is_empty()
            || features.iter().any(|node| {
                !matches!(
                    node.get("operation").and_then(Value::as_str),
                    Some("newBody" | "cut" | "join")
                )
            })
        {
            return None;
        }
        let rollback = self.rollback_index().unwrap_or(features.len());
        let mut cache = self.feature_cache.entries.borrow_mut();
        let mut bodies: Vec<Manifold> = Vec::new();
        let mut body_ids: Vec<String> = Vec::new();
        let mut target: BodyTarget = None;
        let mut fresh = true;
        let mut out = TreeEvaluation {
            previews: Vec::new(),
            statuses: Vec::new(),
            bodies: Vec::new(),
        };
        for (index, node) in features.iter().enumerate() {
            let preview = self.preview_for_extrude(node);
            let key = format!(
                "{}|{}|{}",
                index < rollback,
                node,
                preview.as_ref().map(preview_key).unwrap_or_default()
            );
            if fresh
                && let Some(entry) = cache.get(index)
                && entry.key == key
            {
                bodies.clone_from(&entry.bodies);
                body_ids.clone_from(&entry.body_ids);
                target.clone_from(&entry.target);
                out.previews.push(entry.preview.clone());
                out.statuses.push(entry.status.clone());
                continue;
            }
            fresh = false;
            cache.truncate(index);
            self.feature_cache
                .evaluations
                .set(self.feature_cache.evaluations.get() + 1);
            let (status, preview) = self.evaluate_feature(
                node,
                preview,
                index < rollback,
                &out.statuses,
                &features,
                &mut bodies,
                &mut body_ids,
                &mut target,
            );
            cache.push(CacheEntry {
                key,
                status: status.clone(),
                preview: preview.clone(),
                bodies: bodies.clone(),
                body_ids: body_ids.clone(),
                target: target.clone(),
            });
            out.previews.push(preview);
            out.statuses.push(status);
        }
        cache.truncate(features.len());
        out.bodies = bodies;
        Some(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_feature(
        &self,
        node: &Value,
        preview: Option<SolidPreview>,
        before_rollback: bool,
        statuses: &[FeatureStatus],
        features: &[&Value],
        bodies: &mut Vec<Manifold>,
        body_ids: &mut Vec<String>,
        target: &mut BodyTarget,
    ) -> (FeatureStatus, SolidPreview) {
        let operation = node["operation"].as_str().unwrap_or_default();
        let id = node["id"].as_str().unwrap_or_default().to_string();
        if operation == "newBody" {
            // Later cuts/joins apply to this body even when it is inactive.
            *target = Some((id.clone(), None));
        }
        let fallback = || placeholder(node);
        if !before_rollback {
            return (FeatureStatus::RolledBack, preview.unwrap_or_else(fallback));
        }
        if node.get("suppressed").and_then(Value::as_bool) == Some(true) {
            return (FeatureStatus::Suppressed, preview.unwrap_or_else(fallback));
        }
        let error = |reason: &str, preview: Option<SolidPreview>| {
            (
                FeatureStatus::Error(reason.to_string()),
                preview.unwrap_or_else(fallback),
            )
        };
        let Some(mut preview) = preview else {
            return error("sketch or profile is missing", None);
        };
        if let Some(face) = node["sketchId"]
            .as_str()
            .and_then(|id| self.sketches.iter().find(|sketch| sketch.id == id))
            .and_then(|sketch| sketch.face.as_ref())
        {
            let source = features
                .iter()
                .position(|node| node["id"].as_str() == Some(&face.extrude_id));
            if source.is_none_or(|source| statuses.get(source).is_none_or(|s| !s.is_active())) {
                return error("the sketch face's feature is not active", Some(preview));
            }
        }
        let mut options = match extrude_options(node) {
            Ok(options) => options,
            Err(reason) => return error(&reason, Some(preview)),
        };
        // A cut from a face sketch runs into the body (against the face
        // normal) by default; new bodies and joins grow outward. Flip
        // inverts that default.
        if operation == "cut" && self.is_face_sketch(node) {
            options.flip = !options.flip;
        }
        let body = match operation {
            "newBody" => None,
            _ => match target {
                Some((_, Some(body))) => Some(*body),
                Some((_, None)) => return error("the target body is not active", Some(preview)),
                None => return error("there is no earlier body", Some(preview)),
            },
        };
        if resolve_extent(&mut preview, &options, bodies, body_ids).is_none() {
            return error("the end condition does not reach a body", Some(preview));
        }
        let Ok(tool) = crate::stl::manifold_from_solid(&preview) else {
            return error("the profile does not form a valid solid", Some(preview));
        };
        match body {
            None => {
                bodies.push(tool);
                body_ids.push(id.clone());
                *target = Some((id, Some(bodies.len() - 1)));
            }
            Some(body) => {
                let result = if operation == "cut" {
                    bodies[body].difference(&tool)
                } else {
                    bodies[body].union(&tool)
                };
                if result.status() != ManifoldError::NoError || result.is_empty() {
                    return error("the Boolean result is empty or invalid", Some(preview));
                }
                bodies[body] = result;
            }
        }
        (FeatureStatus::Active, preview)
    }
}
