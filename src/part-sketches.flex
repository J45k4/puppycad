// Cached world-space strokes for the Part workspace. Sketch edits continue
// to use the separate 2D canvas and its own fitting and hit testing.
global puppy_part_sketch_mesh=0;
global puppy_part_sketch_dirty=1;
global puppy_part_sketch_lines=0;
global puppy_part_sketch_points=0;
global puppy_part_sketch_bounds=0;

fn puppy_part_sketch_available() {
    return !puppy_sketch && !puppy_step_input && !puppy_reader_report && !puppy_part_sketch_dirty
        && (puppy_part_sketch_lines || puppy_part_sketch_points);
}
fn puppy_camera_available() {
    return !puppy_sketch && (!puppy_empty || puppy_part_sketch_available());
}
fn puppy_part_sketch_upload(project) {
    puppy_part_sketch_dirty=0;puppy_part_sketch_lines=0;puppy_part_sketch_points=0;
    let part=cad_project_part(project);if !part {return 1;}
    let ids=puppy_part_sketch_ids(part);if !cg_count(ids) {return 1;}
    let paths=cad_project_world_sketch_paths(project,ids);
    // Keep the evaluated model and history available if a sketch has errors.
    if !paths || !cg_count(paths) {return 1;}
    let low=puppy_part_sketch_bounds;let high=low+24;let k=0;
    while k<24 {store64(low+k,500000001);store64(high+k,-500000001);k=k+8;}
    let lines=m_buffer();let markers=m_buffer();let i=0;
    while i<cg_count(paths) {
        let path=cg_at(paths,i);let points=load64(path);
        let rgb=0x8ad7e8;if cad_sketch_is_construction(load64(path+16)) {rgb=0x829aa3;}
        let count=cg_count(points);
        if count==1 {puppy_path_vertex(markers,cg_at(points,0),rgb,low,high);}
        if count>1 {
            let segments=count-1+load64(path+8);let j=0;
            while j<segments {
                puppy_path_vertex(lines,cg_at(points,j),rgb,low,high);
                puppy_path_vertex(lines,cg_at(points,(j+1)%count),rgb,low,high);
                j=j+1;
            }
        }
        i=i+1;
    }
    let line_count=load64(lines+8)/16;let point_count=load64(markers+8)/16;
    if !line_count && !point_count {return 1;}
    if point_count {m_append(lines,m_data(markers),point_count*16);}
    if !puppy_part_sketch_mesh {puppy_part_sketch_mesh=puppy_colored_mesh();}
    if !render_mesh_upload(puppy_part_sketch_mesh,m_data(lines),line_count+point_count,0x88e4) {return 0;}
    puppy_part_sketch_lines=line_count;puppy_part_sketch_points=point_count;
    if puppy_empty {puppy_frame_bounds(low,high);puppy_zoom=280;gui__zero(puppy_pan,24);}
    return 1;
}
fn puppy_part_sketch_fit() {
    if !puppy_part_sketch_available() {
        if !puppy_empty {puppy_frame_bounds(puppy_mesh_bounds,puppy_mesh_bounds+24);}
        return 1;
    }
    let low=m_take(24);let high=m_take(24);let k=0;
    while k<24 {
        let a=load64(puppy_part_sketch_bounds+k);let b=load64(puppy_part_sketch_bounds+24+k);
        if !puppy_empty {
            let solid_low=load64(puppy_mesh_bounds+k);let solid_high=load64(puppy_mesh_bounds+24+k);
            if solid_low<a {a=solid_low;}if solid_high>b {b=solid_high;}
        }
        store64(low+k,a);store64(high+k,b);k=k+8;
    }
    puppy_frame_bounds(low,high);return 1;
}
fn puppy_part_sketch_draw() {
    if !puppy_part_sketch_available() {return 1;}
    let ok=puppy_draw_wire(puppy_part_sketch_mesh,2,1,0,puppy_part_sketch_lines);
    if !ok || !puppy_part_sketch_points {return ok;}
    render_gl(puppy_device,"glEnable",0x8642,0,0,0,0,0);
    ok=puppy_draw_wire(puppy_part_sketch_mesh,2,0,puppy_part_sketch_lines,puppy_part_sketch_points);
    render_gl(puppy_device,"glDisable",0x8642,0,0,0,0,0);return ok;
}
