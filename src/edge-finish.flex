// Keep the picked world-space references between events and rebuild the graph
// once on Apply. Only the selected lines are uploaded during picking.
global puppy_edge_kind=0;
global puppy_edge_refs=0;
global puppy_edge_mesh=0;
global puppy_edge_preview_mesh=0;
global puppy_edge_preview_count=0;
global puppy_edge_tree=0;
global puppy_edge_size=1000000;

fn puppy_edge_label() {if m_eq(puppy_edge_kind,"fillet") {return "Fillet";}return "Chamfer";}
fn puppy_edge_field_name() {if m_eq(puppy_edge_kind,"fillet") {return "Radius";}return "Distance";}
fn puppy_edge_preview(project) {
    puppy_edge_preview_count=0;
    if !puppy_edge_kind || !puppy_edge_refs {return 1;}
    let refs=j_parse(puppy_edge_refs+24);
    if !cg_j_count(refs) {return 1;}
    let candidate=cad_project_add_edge_finish(project,puppy_edge_kind,puppy_edge_size,refs);
    if !candidate {cg_error=0;return 0;}
    let mesh=cad_project_mesh(candidate);
    if !mesh || j_get(mesh,"error") {cg_error=0;return 0;}
    let low=cg_v(0,0,0);let high=cg_v(0,0,0);
    let vertices=puppy_mesh_vertices(mesh,low,high);if !vertices {return 0;}
    let triangles=j_integer(mesh,"triangles");if !triangles {return 0;}
    let data=m_take(triangles*48);let at=0;let part=pc_first(j_get(mesh,"parts"));
    while part {
        let rgb=puppy_part_rgb(part);let id=pc_first(j_get(part,"triangles"));
        while id {
            m_copy(data+at,vertices+j_value(id)*12,12);
            puppy_vertex_color(data+at,rgb);at=at+16;id=load64(id+24);
        }
        part=load64(part+24);
    }
    if !puppy_edge_preview_mesh {puppy_edge_preview_mesh=puppy_colored_mesh();}
    if !render_mesh_upload(puppy_edge_preview_mesh,data,at/16,0x88e8) {return 0;}
    puppy_edge_preview_count=at/16;return 1;
}
fn puppy_edge_draw_preview() {
    if !puppy_edge_kind || !puppy_edge_preview_count {return 0;}
    render_gl(puppy_device,"glUniform1i",render_uniform(puppy_material,0,"moving"),0,0,0,0,0);
    render_gl(puppy_device,"glUniform2i",render_uniform(puppy_material,0,"selection"),0,0,0,0,0);
    return render_mesh_draw(puppy_edge_preview_mesh,puppy_material,0,0,puppy_edge_preview_count);
}
fn puppy_edge_set_size(project,size) {
    if size<=0 || size>500000000 {return puppy_title(project,"Enter a positive size up to 500 mm");}
    puppy_edge_size=size;puppy_number_close();puppy_edge_preview(project);
    return puppy_title(project,m_cat(puppy_edge_field_name(),m_cat(" ",m_cat(puppy_display_number(puppy_edge_size)," mm"))));
}
fn puppy_edge_close() {
    if !puppy_edge_kind {return 0;}
    puppy_edge_kind=0;puppy_edge_preview_count=0;puppy_drop(puppy_edge_refs);puppy_edge_refs=0;
    puppy_tree_open=puppy_edge_tree;puppy_tree_dirty=1;return 1;
}
fn puppy_edge_upload(refs) {
    let buffer=m_buffer();let low=cg_v(0,0,0);let high=cg_v(0,0,0);let row=pc_first(refs);
    while row {
        puppy_path_vertex(buffer,pc_edge_vector(j_get(row,"a")),0xffb044,low,high);
        puppy_path_vertex(buffer,pc_edge_vector(j_get(row,"b")),0xffb044,low,high);
        row=load64(row+24);
    }
    if !puppy_edge_mesh {puppy_edge_mesh=puppy_colored_mesh();}
    return render_mesh_upload(puppy_edge_mesh,m_data(buffer),load64(buffer+8)/16,0x88e8);
}
fn puppy_edge_begin(project,rounded) {
    puppy_search_focus(0);puppy_picking_face=0;puppy_edge_tree=puppy_tree_open;puppy_tree_open=0;
    let initial=0;
    if puppy_selected_edge {
        let refs=j_array();j_add(refs,0,j_parse(puppy_selected_edge+24));
        initial=puppy_snapshot(j_encode(refs));puppy_face_clear();
    }
    puppy_edge_kind="chamfer";if rounded {puppy_edge_kind="fillet";}
    puppy_edge_size=1000000;puppy_edge_preview_count=0;
    if initial {
        puppy_edge_refs=initial;
        if !puppy_edge_upload(j_parse(puppy_edge_refs+24)) {return puppy_title(project,"Cannot draw selected edges");}
        puppy_edge_preview(project);
        return puppy_title(project,"Selected edges 1");
    }
    puppy_edge_refs=puppy_snapshot("[]");
    if puppy_edge_mesh {render_mesh_upload(puppy_edge_mesh,0,0,0x88e8);}
    return puppy_title(project,"Pick straight edges, then Apply");
}
fn puppy_edge_actions() {
    let rows=cg_array();puppy_compact_button(rows,"Chamfer",39,0);puppy_compact_button(rows,"Fillet",39,3);
    puppy_compact_button(rows,"Apply",40,0);puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);
    puppy_compact_button(rows,m_cat(puppy_edge_field_name(),m_cat(" ",m_cat(puppy_display_number(puppy_edge_size)," mm"))),7,0);
    return rows;
}
fn puppy_edge_key(project,key,mods) {
    if key==GUI_KEY_ESCAPE {puppy_edge_close();return puppy_title(project,"Edge finish cancelled");}
    if (key==7 || key==59) && !(mods&0x3c0) {
        return puppy_text_open(project,puppy_display_number(puppy_edge_size));
    }
    if key==39 {
        puppy_edge_kind="chamfer";if mods&3 {puppy_edge_kind="fillet";}
        puppy_edge_preview(project);
        return puppy_title(project,puppy_edge_label());
    }
    if key!=40 {return 1;}
    let refs=j_parse(puppy_edge_refs+24);
    let updated=cad_project_add_edge_finish(project,puppy_edge_kind,puppy_edge_size,refs);
    if !updated {return puppy_title(project,cg_error);}
    let status=m_cat(puppy_edge_label()," created");puppy_edge_close();
    return puppy_finish_feature(project,updated,status);
}
fn puppy_edge_pick(project,x,y) {
    let ray=puppy_ray(x,y);if !ray {return puppy_title(project,cg_error);}
    let distance=puppy_radius*puppy_zoom/100+2;
    let tol=distance*10000000/gui_height(load64(puppy_device));if tol<10000 {tol=10000;}
    if tol>500000000 {tol=500000000;}
    let picked=cad_project_pick_edge_in(project,load64(ray),load64(ray+8),tol,puppy_visible_id());
    if !picked {return puppy_title(project,cg_error);}
    let text=j_encode(picked);let refs=j_parse(puppy_edge_refs+24);let link=refs+16;
    while load64(link) && !m_eq(j_encode(load64(link)),text) {link=load64(link)+24;}
    if load64(link) {store64(link,load64(load64(link)+24));}else {j_add(refs,0,picked);}
    let next=puppy_snapshot(j_encode(refs));puppy_drop(puppy_edge_refs);puppy_edge_refs=next;
    if !puppy_edge_upload(refs) {return puppy_title(project,"Cannot draw selected edges");}
    puppy_edge_preview(project);
    return puppy_title(project,m_cat("Selected edges ",m_int(cg_j_count(refs))));
}
