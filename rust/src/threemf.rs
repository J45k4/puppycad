//! 3MF export: one `.3mf` ZIP per part with an `object` and a `build` item per body.

use std::{
    collections::HashMap,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::stl::{Body, write_numbered};

pub const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
 <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
 <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/>
</Types>
"#;

pub const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
 <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/>
</Relationships>
"#;

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// The `3D/3dmodel.model` XML: vertices are welded per body, units are millimeters.
pub fn model_xml(part_name: &str, bodies: &[Body]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model unit=\"millimeter\" xml:lang=\"en-US\" xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\">\n",
    );
    xml.push_str(&format!(
        " <metadata name=\"Title\">{}</metadata>\n <metadata name=\"Application\">PuppyCAD</metadata>\n <resources>\n",
        escape(part_name)
    ));
    for (index, body) in bodies.iter().enumerate() {
        let mut ids = HashMap::<[u32; 3], usize>::new();
        let mut vertices = String::new();
        let mut triangles = String::new();
        for triangle in &body.triangles {
            let corners = triangle.map(|point| {
                let next = ids.len();
                *ids.entry(point.map(f32::to_bits)).or_insert_with(|| {
                    vertices.push_str(&format!(
                        "     <vertex x=\"{}\" y=\"{}\" z=\"{}\"/>\n",
                        point[0], point[1], point[2]
                    ));
                    next
                })
            });
            if corners[0] != corners[1] && corners[1] != corners[2] && corners[0] != corners[2] {
                triangles.push_str(&format!(
                    "     <triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>\n",
                    corners[0], corners[1], corners[2]
                ));
            }
        }
        xml.push_str(&format!(
            "  <object id=\"{}\" type=\"model\" name=\"{}\">\n   <mesh>\n    <vertices>\n{vertices}    </vertices>\n    <triangles>\n{triangles}    </triangles>\n   </mesh>\n  </object>\n",
            index + 1,
            escape(&body.name)
        ));
    }
    xml.push_str(" </resources>\n <build>\n");
    for index in 0..bodies.len() {
        xml.push_str(&format!("  <item objectid=\"{}\"/>\n", index + 1));
    }
    xml.push_str(" </build>\n</model>\n");
    xml
}

/// The complete 3MF package bytes (deflated ZIP).
pub fn package(part_name: &str, bodies: &[Body]) -> Result<Vec<u8>, String> {
    if bodies.is_empty() {
        return Err("No supported solid to export".into());
    }
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, data) in [
        ("[Content_Types].xml", CONTENT_TYPES.to_string()),
        ("_rels/.rels", RELS.to_string()),
        ("3D/3dmodel.model", model_xml(part_name, bodies)),
    ] {
        zip.start_file(name, options)
            .map_err(|error| format!("3MF {name}: {error}"))?;
        zip.write_all(data.as_bytes())
            .map_err(|error| format!("3MF {name}: {error}"))?;
    }
    zip.finish()
        .map(Cursor::into_inner)
        .map_err(|error| format!("3MF finish: {error}"))
}

/// Writes `<project>.3mf` (numbered when it exists) beside the project file.
pub fn write_3mf(project_path: &Path, part_name: &str, bodies: &[Body]) -> Result<PathBuf, String> {
    write_numbered(project_path, "3mf", &package(part_name, bodies)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::Point,
        project::{SketchPlane, SolidPreview},
        stl::solid_triangles,
    };
    use std::io::Read;

    fn cube(name: &str, offset: f32) -> Body {
        let solid = SolidPreview {
            name: name.into(),
            profile: vec![
                Point { x: offset, y: 0.0 },
                Point {
                    x: offset + 1.0,
                    y: 0.0,
                },
                Point {
                    x: offset + 1.0,
                    y: 1.0,
                },
                Point { x: offset, y: 1.0 },
            ],
            holes: Vec::new(),
            depth: 1.0,
            plane: SketchPlane::Top,
        };
        Body::new(name, solid_triangles(&solid).unwrap())
    }

    #[test]
    fn package_lists_required_parts_and_model_counts_match() {
        let bodies = [cube("A & B", 0.0), cube("Second", 5.0)];
        let bytes = package("Part <1>", &bodies).unwrap();
        assert_eq!(&bytes[..2], b"PK");
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut names: Vec<&str> = archive.file_names().collect();
        names.sort();
        assert_eq!(
            names,
            ["3D/3dmodel.model", "[Content_Types].xml", "_rels/.rels"]
        );
        let mut model = String::new();
        let mut file = archive.by_name("3D/3dmodel.model").unwrap();
        assert_eq!(file.compression(), CompressionMethod::Deflated);
        file.read_to_string(&mut model).unwrap();
        assert!(model.contains("unit=\"millimeter\""));
        assert!(
            model.contains("xmlns=\"http://schemas.microsoft.com/3dmanufacturing/core/2015/02\"")
        );
        assert!(model.contains("<metadata name=\"Title\">Part &lt;1&gt;</metadata>"));
        assert!(model.contains("name=\"A &amp; B\""));
        assert_eq!(model.matches("<object ").count(), 2);
        assert_eq!(model.matches("<vertex ").count(), 16);
        assert_eq!(model.matches("<triangle ").count(), 24);
        assert_eq!(model.matches("<item objectid=").count(), 2);
        // Element balance as a tiny well-formedness check.
        for tag in [
            "model",
            "resources",
            "object",
            "mesh",
            "vertices",
            "triangles",
            "build",
        ] {
            assert_eq!(
                model.matches(&format!("<{tag}>")).count()
                    + model.matches(&format!("<{tag} ")).count(),
                model.matches(&format!("</{tag}>")).count(),
                "{tag}"
            );
        }
        // Each triangle index refers to a vertex inside its own object.
        for object in model.split("<object ").skip(1) {
            let vertices = object.matches("<vertex ").count();
            for line in object.lines().filter(|line| line.contains("<triangle ")) {
                for value in line.split('"').skip(1).step_by(2) {
                    assert!(value.parse::<usize>().unwrap() < vertices);
                }
            }
        }
    }

    #[test]
    fn empty_part_is_rejected() {
        assert!(package("Empty", &[]).is_err());
    }
}
