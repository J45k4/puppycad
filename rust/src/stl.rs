use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use crate::project::SolidPreview;
use manifold_rust::{
    manifold::Manifold,
    types::{Error as ManifoldError, MeshGL},
};
use std::collections::HashMap;

type Vertex = [f32; 3];

fn triangle(lines: &mut Vec<String>, a: Vertex, b: Vertex, c: Vertex) {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let mut normal = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length < 1e-6 {
        return;
    }
    for value in &mut normal {
        *value /= length;
    }
    lines.push(format!(
        "facet normal {} {} {}",
        normal[0], normal[1], normal[2]
    ));
    lines.push("outer loop".into());
    for p in [a, b, c] {
        lines.push(format!("vertex {} {} {}", p[0], p[1], p[2]));
    }
    lines.push("endloop".into());
    lines.push("endfacet".into());
}

pub fn export_solid(solid: &SolidPreview) -> Result<String, String> {
    let triangles = solid_triangles(solid)?;
    let mut lines = vec!["solid puppycad".into()];
    for [a, b, c] in triangles {
        triangle(&mut lines, a, b, c);
    }
    lines.push("endsolid puppycad".into());
    Ok(lines.join("\n"))
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

pub fn write_export(project_path: &Path, solid: &SolidPreview) -> Result<PathBuf, String> {
    let stl = export_solid(solid)?;
    write_stl(project_path, &stl)
}

pub fn write_mesh_export(project_path: &Path, solid: &Manifold) -> Result<PathBuf, String> {
    let mesh = solid.get_mesh_gl(0);
    let points: Vec<Vertex> = mesh
        .vert_properties
        .chunks_exact(mesh.num_prop as usize)
        .map(|point| [point[0], point[1], point[2]])
        .collect();
    let mut lines = vec!["solid puppycad".into()];
    for indices in mesh.tri_verts.as_chunks::<3>().0 {
        triangle(
            &mut lines,
            points[indices[0] as usize],
            points[indices[1] as usize],
            points[indices[2] as usize],
        );
    }
    lines.push("endsolid puppycad".into());
    write_stl(project_path, &lines.join("\n"))
}

fn write_stl(project_path: &Path, stl: &str) -> Result<PathBuf, String> {
    let base = project_path.with_extension("stl");
    for index in 1..1000 {
        let path = if index == 1 {
            base.clone()
        } else {
            base.with_file_name(format!(
                "{}-{index}.stl",
                project_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("puppycad")
            ))
        };
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(stl.as_bytes()) {
                    let _ = std::fs::remove_file(&path);
                    return Err(format!("write STL: {error}"));
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create STL: {error}")),
        }
    }
    Err("too many STL exports with this project name".into())
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
