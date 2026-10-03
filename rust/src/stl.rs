use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use crate::project::{ProjectFile, SolidPreview};
use manifold_rust::{
    manifold::Manifold,
    types::{Error as ManifoldError, MeshGL},
};
use std::collections::HashMap;

type Vertex = [f32; 3];

fn facet_normal([a, b, c]: &[Vertex; 3]) -> Option<Vertex> {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let normal = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    (length >= 1e-6).then(|| normal.map(|value| value / length))
}

/// One exportable body: a display name and its non-degenerate triangles.
#[derive(Clone, Debug)]
pub struct Body {
    pub name: String,
    pub triangles: Vec<[Vertex; 3]>,
}

impl Body {
    pub fn new(name: impl Into<String>, triangles: Vec<[Vertex; 3]>) -> Self {
        Self {
            name: name.into(),
            triangles: triangles
                .into_iter()
                .filter(|triangle| facet_normal(triangle).is_some())
                .collect(),
        }
    }

    pub fn from_manifold(name: impl Into<String>, solid: &Manifold) -> Self {
        let mesh = solid.get_mesh_gl(0);
        let points: Vec<Vertex> = mesh
            .vert_properties
            .chunks_exact(mesh.num_prop as usize)
            .map(|point| [point[0], point[1], point[2]])
            .collect();
        let triangles = mesh
            .tri_verts
            .as_chunks::<3>()
            .0
            .iter()
            .map(|indices| indices.map(|index| points[index as usize]))
            .collect();
        Self::new(name, triangles)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StlFormat {
    Ascii,
    Binary,
}

/// ASCII STL text for a triangle list; normals are computed per facet.
pub fn ascii_stl(triangles: &[[Vertex; 3]]) -> String {
    let mut lines = vec!["solid puppycad".to_string()];
    for triangle in triangles {
        let Some(normal) = facet_normal(triangle) else {
            continue;
        };
        lines.push(format!(
            "facet normal {} {} {}",
            normal[0], normal[1], normal[2]
        ));
        lines.push("outer loop".into());
        for p in triangle {
            lines.push(format!("vertex {} {} {}", p[0], p[1], p[2]));
        }
        lines.push("endloop".into());
        lines.push("endfacet".into());
    }
    lines.push("endsolid puppycad".into());
    lines.join("\n")
}

/// Binary STL: 80-byte header, little-endian u32 count, 50 bytes per facet.
/// The header never starts with `solid` so readers do not mistake it for ASCII.
pub fn binary_stl(header: &str, triangles: &[[Vertex; 3]]) -> Vec<u8> {
    let facets: Vec<(Vertex, &[Vertex; 3])> = triangles
        .iter()
        .filter_map(|triangle| facet_normal(triangle).map(|normal| (normal, triangle)))
        .collect();
    let mut bytes = Vec::with_capacity(84 + facets.len() * 50);
    let mut head = [b' '; 80];
    let text = format!("PuppyCAD {header}");
    let text = text.as_bytes();
    let length = text.len().min(80);
    head[..length].copy_from_slice(&text[..length]);
    bytes.extend_from_slice(&head);
    bytes.extend_from_slice(&(facets.len() as u32).to_le_bytes());
    for (normal, triangle) in facets {
        for value in normal.iter().chain(triangle.iter().flatten()) {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0u16.to_le_bytes());
    }
    bytes
}

/// Every supported body of the project's active part, preferring evaluated
/// boolean results when cuts/joins merged extrusions.
pub fn project_bodies(project: &ProjectFile) -> Result<Vec<Body>, String> {
    let solids = project
        .extrusion_previews()
        .ok_or("No supported solid to export")?;
    match project.evaluated_solids() {
        Some(meshes) if meshes.len() < solids.len() => Ok(meshes
            .iter()
            .enumerate()
            .map(|(index, mesh)| Body::from_manifold(format!("Body {}", index + 1), mesh))
            .collect()),
        _ => solids
            .iter()
            .map(|solid| Ok(Body::new(solid.name.clone(), solid_triangles(solid)?)))
            .collect(),
    }
}

pub fn export_solid(solid: &SolidPreview) -> Result<String, String> {
    Ok(ascii_stl(&solid_triangles(solid)?))
}

pub fn solid_triangles(solid: &SolidPreview) -> Result<Vec<[Vertex; 3]>, String> {
    if !solid.depth.is_finite()
        || solid.depth <= 0.0
        || solid.loops().any(|loop_points| {
            loop_points.len() < 3
                || loop_points
                    .iter()
                    .any(|p| !p.x.is_finite() || !p.y.is_finite())
        })
    {
        return Err("solid geometry is invalid for STL export".into());
    }
    let (cap_points, cap_triangles) = solid.cap_mesh().ok_or("solid cap cannot be triangulated")?;
    let mut triangles = Vec::new();
    for (loop_index, loop_points) in solid.loops().enumerate() {
        let signed_area: f32 = loop_points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let q = loop_points[(i + 1) % loop_points.len()];
                p.x * q.y - q.x * p.y
            })
            .sum();
        let reverse = (signed_area > 0.0) != (loop_index == 0);
        for i in 0..loop_points.len() {
            let j = (i + 1) % loop_points.len();
            let [a, b, c, d] = [
                solid.world_point(loop_points[i], 0.0),
                solid.world_point(loop_points[j], 0.0),
                solid.world_point(loop_points[j], solid.depth),
                solid.world_point(loop_points[i], solid.depth),
            ];
            if reverse {
                triangles.push([a, c, b]);
                triangles.push([a, d, c]);
            } else {
                triangles.push([a, b, c]);
                triangles.push([a, c, d]);
            }
        }
    }
    for [i, j, k] in cap_triangles {
        let [a, b, c] = [cap_points[i], cap_points[j], cap_points[k]];
        let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        let [a_top, b_top, c_top] = [
            solid.world_point(a, solid.depth),
            solid.world_point(b, solid.depth),
            solid.world_point(c, solid.depth),
        ];
        let [a_bottom, b_bottom, c_bottom] = [
            solid.world_point(a, 0.0),
            solid.world_point(b, 0.0),
            solid.world_point(c, 0.0),
        ];
        if cross > 0.0 {
            triangles.push([a_top, b_top, c_top]);
            triangles.push([a_bottom, c_bottom, b_bottom]);
        } else {
            triangles.push([a_top, c_top, b_top]);
            triangles.push([a_bottom, b_bottom, c_bottom]);
        }
    }
    if solid.frame.mirrored() {
        for triangle in &mut triangles {
            triangle.swap(1, 2);
        }
    }
    Ok(triangles)
}

pub fn manifold_from_solid(solid: &SolidPreview) -> Result<Manifold, String> {
    let triangles = solid_triangles(solid)?;
    let mut mesh = MeshGL {
        num_prop: 3,
        ..MeshGL::default()
    };
    let mut indices = HashMap::<[u32; 3], u32>::new();
    for triangle in triangles {
        for vertex in triangle {
            let key = vertex.map(f32::to_bits);
            let index = *indices.entry(key).or_insert_with(|| {
                let index = (mesh.vert_properties.len() / 3) as u32;
                mesh.vert_properties.extend(vertex);
                index
            });
            mesh.tri_verts.push(index);
        }
    }
    let manifold = Manifold::from_mesh_gl(&mesh);
    if manifold.status() != ManifoldError::NoError {
        return Err(format!(
            "extrusion mesh is not manifold: {:?}",
            manifold.status()
        ));
    }
    Ok(manifold)
}

/// Writes one STL per body beside `project_path` (numbered, never overwriting).
pub fn write_stl_bodies(
    project_path: &Path,
    part_name: &str,
    bodies: &[Body],
    format: StlFormat,
) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for body in bodies {
        let bytes = match format {
            StlFormat::Ascii => ascii_stl(&body.triangles).into_bytes(),
            StlFormat::Binary => {
                binary_stl(&format!("{part_name} / {}", body.name), &body.triangles)
            }
        };
        match write_numbered(project_path, "stl", &bytes) {
            Ok(path) => paths.push(path),
            Err(reason) => {
                return Err(format!(
                    "STL export failed after {} bodies: {reason}",
                    paths.len()
                ));
            }
        }
    }
    Ok(paths)
}

/// Creates `<stem>.<ext>`, or `<stem>-N.<ext>` when that already exists.
pub fn write_numbered(
    project_path: &Path,
    extension: &str,
    bytes: &[u8],
) -> Result<PathBuf, String> {
    let base = project_path.with_extension(extension);
    for index in 1..1000 {
        let path = if index == 1 {
            base.clone()
        } else {
            base.with_file_name(format!(
                "{}-{index}.{extension}",
                project_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("puppycad")
            ))
        };
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    let _ = std::fs::remove_file(&path);
                    return Err(format!("write {}: {error}", path.display()));
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create {}: {error}", path.display())),
        }
    }
    Err("too many exports with this project name".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{model::Point, project::SketchPlane};
    use std::collections::HashMap;

    #[test]
    fn rectangle_stl_is_closed_and_has_outward_normals() {
        let solid = SolidPreview {
            name: "Block".into(),
            profile: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                Point { x: 10.0, y: 5.0 },
                Point { x: 0.0, y: 5.0 },
            ],
            depth: 20.0,
            frame: SketchPlane::Top.frame(),
            holes: Vec::new(),
        };
        let stl = export_solid(&solid).unwrap();
        assert_eq!(stl.matches("facet normal").count(), 12);
        assert!(stl.contains("facet normal 0 0 1"));
        assert!(stl.contains("facet normal 0 0 -1"));
        assert!(stl.contains("facet normal 0 -1 0"));
        assert!(stl.contains("facet normal 1 0 0"));
        let manifold = manifold_from_solid(&solid).unwrap();
        assert!((manifold.volume() - 1000.0).abs() < 1e-2);
        let reversed = SolidPreview {
            profile: solid.profile.iter().rev().copied().collect(),
            ..solid
        };
        let reversed_stl = export_solid(&reversed).unwrap();
        assert_eq!(reversed_stl.matches("facet normal").count(), 12);
        assert!(reversed_stl.contains("facet normal 0 0 1"));
    }

    #[test]
    fn manifold_difference_cuts_a_through_hole_in_an_extrusion() {
        let base = SolidPreview {
            name: "Block".into(),
            profile: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                Point { x: 10.0, y: 10.0 },
                Point { x: 0.0, y: 10.0 },
            ],
            holes: Vec::new(),
            depth: 10.0,
            frame: SketchPlane::Top.frame(),
        };
        let cutter = SolidPreview {
            name: "Cutter".into(),
            profile: vec![
                Point { x: 3.0, y: 3.0 },
                Point { x: 7.0, y: 3.0 },
                Point { x: 7.0, y: 7.0 },
                Point { x: 3.0, y: 7.0 },
            ],
            holes: Vec::new(),
            depth: 10.0,
            frame: SketchPlane::Top.frame(),
        };
        let result = manifold_from_solid(&base)
            .unwrap()
            .difference(&manifold_from_solid(&cutter).unwrap());
        assert_eq!(result.status(), ManifoldError::NoError);
        assert!((result.volume() - 840.0).abs() < 1e-2);
    }

    #[test]
    fn front_and_right_stl_vertices_follow_plane_frames() {
        for plane in [SketchPlane::Front, SketchPlane::Right] {
            let solid = SolidPreview {
                name: "Block".into(),
                profile: vec![
                    Point { x: 10.0, y: 20.0 },
                    Point { x: 50.0, y: 20.0 },
                    Point { x: 50.0, y: 40.0 },
                    Point { x: 10.0, y: 40.0 },
                ],
                depth: 30.0,
                frame: plane.frame(),
                holes: Vec::new(),
            };
            let stl = export_solid(&solid).unwrap();
            assert_eq!(stl.matches("facet normal").count(), 12);
            if plane == SketchPlane::Front {
                assert!(stl.contains("vertex 10 30 -20"));
                assert!(stl.contains("facet normal 0 1 0"));
            } else {
                assert!(stl.contains("vertex 30 10 20"));
                assert!(stl.contains("facet normal 1 0 0"));
            }
        }
    }

    #[test]
    fn hollow_extrusion_stl_is_watertight_and_has_expected_volume() {
        let solid = SolidPreview {
            name: "Hollow block".into(),
            profile: vec![
                Point { x: -50.0, y: -40.0 },
                Point { x: 50.0, y: -40.0 },
                Point { x: 50.0, y: 40.0 },
                Point { x: -50.0, y: 40.0 },
            ],
            holes: vec![
                vec![
                    Point { x: -10.0, y: -10.0 },
                    Point { x: -10.0, y: 10.0 },
                    Point { x: 10.0, y: 10.0 },
                    Point { x: 10.0, y: -10.0 },
                ],
                vec![
                    Point { x: 20.0, y: -10.0 },
                    Point { x: 20.0, y: 10.0 },
                    Point { x: 40.0, y: 10.0 },
                    Point { x: 40.0, y: -10.0 },
                ],
            ],
            depth: 10.0,
            frame: SketchPlane::Top.frame(),
        };
        let stl = export_solid(&solid).unwrap();
        let vertices: Vec<Vec<f64>> = stl
            .lines()
            .filter(|line| line.starts_with("vertex "))
            .map(|line| {
                line.split_whitespace()
                    .skip(1)
                    .map(|v| v.parse().unwrap())
                    .collect()
            })
            .collect();
        let mut edges: HashMap<(String, String), usize> = HashMap::new();
        let mut volume = 0.0;
        for triangle in vertices.as_chunks::<3>().0 {
            let [a, b, c] = [&triangle[0], &triangle[1], &triangle[2]];
            volume += (a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
            for index in 0..3 {
                let key = |point: &Vec<f64>| format!("{},{},{}", point[0], point[1], point[2]);
                let mut pair = [key(&triangle[index]), key(&triangle[(index + 1) % 3])];
                pair.sort();
                *edges.entry((pair[0].clone(), pair[1].clone())).or_default() += 1;
            }
        }
        assert!(
            edges.values().all(|count| *count == 2),
            "{:?}",
            edges
                .iter()
                .filter(|(_, count)| **count != 2)
                .collect::<Vec<_>>()
        );
        assert!((volume - 72000.0_f64).abs() < 0.01);
    }

    #[test]
    fn concave_line_profile_exports_a_watertight_stl() {
        let solid = SolidPreview {
            name: "L bracket".into(),
            profile: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 60.0, y: 0.0 },
                Point { x: 60.0, y: 20.0 },
                Point { x: 20.0, y: 20.0 },
                Point { x: 20.0, y: 50.0 },
                Point { x: 0.0, y: 50.0 },
            ],
            holes: Vec::new(),
            depth: 10.0,
            frame: SketchPlane::Top.frame(),
        };
        let stl = export_solid(&solid).unwrap();
        let vertices: Vec<Vec<f64>> = stl
            .lines()
            .filter(|line| line.starts_with("vertex "))
            .map(|line| {
                line.split_whitespace()
                    .skip(1)
                    .map(|value| value.parse().unwrap())
                    .collect()
            })
            .collect();
        let mut edges: HashMap<(String, String), usize> = HashMap::new();
        let mut volume = 0.0;
        for triangle in vertices.as_chunks::<3>().0 {
            let [a, b, c] = [&triangle[0], &triangle[1], &triangle[2]];
            volume += (a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
            for index in 0..3 {
                let key = |point: &Vec<f64>| format!("{},{},{}", point[0], point[1], point[2]);
                let mut pair = [key(&triangle[index]), key(&triangle[(index + 1) % 3])];
                pair.sort();
                *edges.entry((pair[0].clone(), pair[1].clone())).or_default() += 1;
            }
        }
        assert!(edges.values().all(|count| *count == 2));
        assert!((volume - 18000.0).abs() < 0.01);
    }
}

#[cfg(test)]
mod binary_tests {
    use super::*;
    use crate::{model::Point, project::SketchPlane};

    fn unit_cube() -> SolidPreview {
        SolidPreview {
            name: "Cube".into(),
            profile: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 1.0, y: 0.0 },
                Point { x: 1.0, y: 1.0 },
                Point { x: 0.0, y: 1.0 },
            ],
            holes: Vec::new(),
            depth: 1.0,
            frame: SketchPlane::Top.frame(),
        }
    }

    fn f32_at(bytes: &[u8], offset: usize) -> f32 {
        f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn binary_stl_layout_for_a_unit_cube() {
        let triangles = solid_triangles(&unit_cube()).unwrap();
        let bytes = binary_stl("Part 1 / Cube", &triangles);
        assert_eq!(bytes.len(), 84 + 12 * 50);
        let header = std::str::from_utf8(&bytes[..80]).unwrap();
        assert!(header.starts_with("PuppyCAD Part 1 / Cube"));
        assert_eq!(u32::from_le_bytes(bytes[80..84].try_into().unwrap()), 12);
        let first = triangles[0];
        let normal = facet_normal(&first).unwrap();
        for (index, expected) in normal.iter().chain(first.iter().flatten()).enumerate() {
            assert_eq!(f32_at(&bytes, 84 + index * 4), *expected);
        }
        assert_eq!(&bytes[84 + 48..84 + 50], &[0, 0]);
        let normal_length = normal.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((normal_length - 1.0).abs() < 1e-6);
    }

    #[test]
    fn ascii_and_binary_stl_contain_identical_triangles() {
        let triangles = solid_triangles(&unit_cube()).unwrap();
        let ascii: Vec<f32> = ascii_stl(&triangles)
            .lines()
            .filter_map(|line| line.strip_prefix("vertex "))
            .flat_map(|rest| rest.split_whitespace().map(|v| v.parse::<f32>().unwrap()))
            .collect();
        let binary = binary_stl("cube", &triangles);
        let count = u32::from_le_bytes(binary[80..84].try_into().unwrap()) as usize;
        let mut vertices = Vec::new();
        for facet in 0..count {
            for index in 3..12 {
                vertices.push(f32_at(&binary, 84 + facet * 50 + index * 4));
            }
        }
        assert_eq!(ascii, vertices);
    }

    #[test]
    fn degenerate_triangles_are_dropped_from_bodies() {
        let body = Body::new("flat", vec![[[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]]);
        assert!(body.triangles.is_empty());
        assert_eq!(binary_stl("flat", &body.triangles).len(), 84);
    }
}
