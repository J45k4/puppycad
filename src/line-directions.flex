import "cad/project-line-directions.flex";

global puppy_direction_mode=0;
global puppy_direction_source=0;

fn puppy_direction_end() {
    if puppy_direction_source {puppy_drop(puppy_direction_source);puppy_direction_source=0;}
    puppy_direction_mode=0;return 1;
}
fn puppy_direction_available() {
    if !puppy_line_report || puppy_direction_mode || puppy_symmetry_mode || puppy_distance_picking || puppy_line_drag {return 0;}
    let first=pc_first(j_get(j_parse(puppy_line_report+24),"lines"));return first && load64(first+24);
}
fn puppy_direction_button_left(w) {return w-128;}
fn puppy_direction_draw(rgb,w) {
    let x=puppy_direction_button_left(w);
    puppy_bar_rect(rgb,w,x,7,116,26,48,68,70);return puppy_bar_text(rgb,w,x+8,25,"Line relations",w-12);
}
fn puppy_direction_name() {
    if puppy_direction_mode==2 {return "Parallel";}
    if puppy_direction_mode==3 {return "Perpendicular";}
    return "Equal";
}
fn puppy_direction_hint() {
    if puppy_direction_mode==1 {return "Choose a line relation";}
    let pick="Pick the source line for ";if puppy_direction_source {pick="Pick the follower line for ";}
    return m_cat(pick,puppy_direction_name());
}
fn puppy_direction_actions(buttons) {
    puppy_compact_button(buttons,"Parallel",19,0);puppy_compact_button(buttons,"Perpendicular",27,0);
    puppy_compact_button(buttons,"Equal",8,0);puppy_compact_button(buttons,"Cancel",GUI_KEY_ESCAPE,0);return buttons;
}
fn puppy_direction_command(project,key) {
    if key==GUI_KEY_ESCAPE {puppy_direction_end();return puppy_title(project,puppy_line_label());}
    if key==15 || key==19 || key==27 || key==8 {
        puppy_direction_end();puppy_bar_page=0;puppy_direction_mode=1;
        if key==19 {puppy_direction_mode=2;}if key==27 {puppy_direction_mode=3;}if key==8 {puppy_direction_mode=4;}
    }
    return puppy_title(project,puppy_direction_hint());
}
fn puppy_direction_pick(project,x,y) {
    if puppy_direction_mode==1 {return 1;}
    let sk=j_string(puppy_draft_sketch(project),"id");let nodes=pc_nodes(cad_project_part(project));
    let picked=puppy_mirror_hit(nodes,sk,puppy_draft_point(x,y),1);
    if !picked {return puppy_title(project,puppy_direction_hint());}
    let id=j_string(picked,"id");
    if !puppy_direction_source {puppy_direction_source=puppy_snapshot(id);return puppy_title(project,puppy_direction_hint());}
    if m_eq(id,puppy_direction_source+24) {return puppy_title(project,"Pick a different follower line");}
    let label=puppy_direction_name();let kind="parallel";if puppy_direction_mode==3 {kind="perpendicular";}
    // Refreshing the report releases the picker snapshot before history is written.
    let source=m_slice(puppy_direction_source+24,load64(puppy_direction_source+16));
    let updated=0;
    if puppy_direction_mode==4 {updated=cad_project_toggle_equal_lines(project,sk,source,id);}
    else {updated=cad_project_toggle_line_direction(project,sk,source,id,kind);}
    if !updated || !puppy_line_update(project,updated) {return puppy_title(project,cg_error);}
    let rows=j_get(j_parse(puppy_line_report+24),"lines");let row=pc_first(rows);let i=0;
    while row && !m_eq(j_string(row,"id"),id) {i=i+1;row=load64(row+24);}
    if row {puppy_line_index=i;puppy_distance_index=-1;}
    return puppy_title(updated,m_cat(label," updated"));
}
