//! Assembly documents inside a project: listing, reading and writing the
//! `data` of top-level `"type": "assembly"` items (see `crate::assembly`),
//! and evaluating any part's bodies for the assembly view and export.

use super::*;

use crate::assembly::Assembly;
use crate::stl::{self, Body};

impl ProjectFile {
    /// Item indices of top-level assembly documents.
    pub fn assembly_indices(&self) -> Vec<usize> {
        self.value["items"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, item)| (item["type"] == "assembly").then_some(index))
            .collect()
    }

    /// Name of any top-level item.
    pub fn item_name_at(&self, index: usize) -> Option<&str> {
        self.value["items"].get(index)?.get("name")?.as_str()
    }

    /// `(id, name)` of every top-level part, in project order.
    pub fn parts(&self) -> Vec<(String, String)> {
        self.part_indices()
            .into_iter()
            .filter_map(|index| {
                let item = &self.value["items"][index];
                Some((
                    item.get("id")?.as_str()?.to_string(),
                    item.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("Part")
                        .to_string(),
                ))
            })
            .collect()
    }

    pub fn assembly_at(&self, index: usize) -> Option<Assembly> {
        let item = self.value["items"].get(index)?;
        (item.get("type")?.as_str()? == "assembly").then(|| {
            Assembly::from_json(
                item.get("data"),
                item.get("id").and_then(Value::as_str).unwrap_or("assembly"),
                item.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Assembly"),
            )
        })
    }

    /// Replaces an assembly item's data. Read-only assemblies are refused so
    /// unsupported content survives untouched.
    pub fn set_assembly(&mut self, index: usize, assembly: &Assembly) -> Result<(), String> {
        if let Some(reason) = &assembly.read_only_reason {
            return Err(format!("assembly is read-only: {reason}"));
        }
        let item = self.value["items"]
            .get_mut(index)
            .filter(|item| item["type"] == "assembly")
            .ok_or("assembly does not exist")?;
        item["data"] = assembly.to_json();
        self.metadata_dirty = true;
        Ok(())
    }

    /// Appends an empty assembly document and returns its item index.
    pub fn create_assembly(&mut self) -> Result<usize, String> {
        let items = self.value["items"]
            .as_array_mut()
            .ok_or("project has no items array")?;
        let mut number = 1;
        while items.iter().any(|item| {
            item.get("id").and_then(Value::as_str)
                == Some(format!("native-assembly-{number}").as_str())
        }) {
            number += 1;
        }
        let id = format!("native-assembly-{number}");
        let name = format!("Assembly {number}");
        items.push(json!({
            "id": id, "type": "assembly", "name": name,
            "data": {"id": id, "name": name, "instances": []}
        }));
        self.metadata_dirty = true;
        Ok(items.len() - 1)
    }

    /// The project's selected top-level item when it is an assembly.
    pub fn selected_assembly(&self) -> Option<usize> {
        let index =
            usize::try_from(self.value["selectedPath"].as_array()?.first()?.as_u64()?).ok()?;
        self.assembly_indices().contains(&index).then_some(index)
    }

    /// The assembly item whose name (or id) is `name`.
    pub fn assembly_named(&self, name: &str) -> Option<usize> {
        self.assembly_indices().into_iter().find(|&index| {
            let item = &self.value["items"][index];
            item.get("name").and_then(Value::as_str) == Some(name)
                || item.get("id").and_then(Value::as_str) == Some(name)
        })
    }

    /// Every instance of `assembly` as one world-space body named after it.
    /// The assembly is expected to be solved already.
    pub fn assembly_world_bodies(
        &self,
        assembly: &Assembly,
        document: &Document,
    ) -> Result<Vec<Body>, String> {
        let mut parts = std::collections::HashMap::new();
        let mut out = Vec::new();
        for instance in &assembly.instances {
            let bodies = parts
                .entry(instance.part_id.clone())
                .or_insert_with(|| self.part_bodies(&instance.part_id, document))
                .as_ref()
                .map_err(|error| format!("{}: {error}", instance.label()))?;
            let transform = instance.transform();
            let triangles = bodies
                .iter()
                .flat_map(|body| &body.triangles)
                .map(|triangle| {
                    triangle.map(|p| transform.apply(p.map(f64::from)).map(|c| c as f32))
                })
                .collect();
            out.push(Body::new(instance.label(), triangles));
        }
        if out.is_empty() {
            return Err("Assembly has no instances to export".into());
        }
        Ok(out)
    }

    /// Evaluated bodies of the part with item id `part_id`, including unsaved
    /// edits of the open sketch `document`.
    pub fn part_bodies(&self, part_id: &str, document: &Document) -> Result<Vec<Body>, String> {
        let (mut value, _, _) = self.staged(document)?;
        let index = value["items"]
            .as_array()
            .and_then(|items| {
                items.iter().position(|item| {
                    item["type"] == "part"
                        && item.get("id").and_then(Value::as_str) == Some(part_id)
                })
            })
            .ok_or_else(|| format!("part \"{part_id}\" does not exist"))?;
        value["selectedPath"] = json!([index]);
        let (part, _) = Self::from_value(self.path.clone(), value, None)?;
        stl::project_bodies(&part)
    }
}
