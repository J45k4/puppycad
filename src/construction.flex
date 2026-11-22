// A selection belongs to this tool until Apply or Cancel.
global puppy_construction_ids=0;

fn puppy_construction_actions() {
    let rows=cg_array();
    puppy_compact_button(rows,"Apply",40,0);
    puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);
    return rows;
}
fn puppy_construction_key(project,key) {
    if key!=40 && key!=GUI_KEY_ESCAPE {return 1;}
    let status="Construction edit cancelled";
    if key==40 {
        let sketch=j_string(puppy_draft_sketch(project),"id");
        let ids=j_parse(puppy_construction_ids+24);
        let updated=cad_project_toggle_construction(project,sketch,ids);
        if !updated {return puppy_title(project,cg_error);}
        let first=pc_find(pc_nodes(cad_project_part(updated)),j_value(pc_first(ids)));
        status="Profile geometry";if cad_sketch_is_construction(first) {status="Construction geometry";}
        project=updated;puppy_push(project);
    }
    puppy_drop(puppy_construction_ids);puppy_construction_ids=0;
    return puppy_title(project,status);
}
fn puppy_construction_pick(project,x,y) {
    let sketch=j_string(puppy_draft_sketch(project),"id");
    let node=puppy_sketch_hit(pc_nodes(cad_project_part(project)),sketch,puppy_draft_point(x,y));
    if !node {return puppy_title(project,"Pick sketch geometry");}
    let ids=j_parse(puppy_construction_ids+24);let id=j_string(node,"id");let selected=pc_first(ids);
    while selected && !m_eq(j_value(selected),id) {selected=load64(selected+24);}
    if selected {pc_line_unlink(ids,selected,id);}else {j_add(ids,0,j_text(id));}
    puppy_drop(puppy_construction_ids);puppy_construction_ids=puppy_snapshot(j_encode(ids));
    return puppy_title(project,m_cat(m_int(cg_j_count(ids))," selected   Apply to toggle construction"));
}
