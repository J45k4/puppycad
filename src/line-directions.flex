import "cad/project-line-directions.flex";
import "cad/project-concentric.flex";

global puppy_direction_mode=0;
global puppy_direction_source=0;

fn puppy_direction_end() {
    if puppy_direction_source {puppy_drop(puppy_direction_source);puppy_direction_source=0;}
    puppy_direction_mode=0;return 1;
}
fn puppy_direction_has_lines() {
    let first=pc_first(j_get(j_parse(puppy_line_report+24),"lines"));return first && load64(first+24);
}
fn puppy_direction_has_curves(project) {
    let nodes=pc_nodes(cad_project_part(project));let sk=j_string(puppy_draft_sketch(project),"id");
    let row=pc_first(nodes);let count=0;
    while row {
        if m_eq(j_string(row,"sketchId"),sk) && pc_concentric_curve(nodes,sk,j_string(row,"id")) {
            count=count+1;if count==2 {return 1;}
        }
        row=load64(row+24);
    }
    return 0;
}
fn puppy_direction_available() {
    if !puppy_line_report || puppy_direction_mode || puppy_symmetry_mode || puppy_distance_picking || puppy_line_drag {return 0;}
    return puppy_direction_has_lines() || puppy_direction_has_curves(cad_project_open(puppy_history+24));
}
fn puppy_direction_button_left(w) {return w-128;}
fn puppy_direction_draw(rgb,w) {
    let x=puppy_direction_button_left(w);
    puppy_bar_rect(rgb,w,x,7,116,26,48,68,70);return puppy_bar_text(rgb,w,x+8,25,"Relations",w-12);
}
fn puppy_direction_name() {
    if puppy_direction_mode==2 {return "Parallel";}
    if puppy_direction_mode==3 {return "Perpendicular";}
    if puppy_direction_mode==4 {return "Equal";}
    return "Concentric";
}
fn puppy_direction_hint() {
    if puppy_direction_mode==1 {return "Choose a relation";}
    let pick="Pick the source ";if puppy_direction_source {pick="Pick the follower ";}
    let entity="line for ";if puppy_direction_mode==5 {entity="curve for ";}
    return m_cat(pick,m_cat(entity,puppy_direction_name()));
}
fn puppy_direction_actions(buttons) {
    if puppy_direction_has_lines() {
        puppy_compact_button(buttons,"Parallel",19,0);puppy_compact_button(buttons,"Perpendicular",27,0);puppy_compact_button(buttons,"Equal",8,0);
    }
    if puppy_direction_has_curves(cad_project_open(puppy_history+24)) {puppy_compact_button(buttons,"Concentric",6,0);}
    puppy_compact_button(buttons,"Cancel",GUI_KEY_ESCAPE,0);return buttons;
}
fn puppy_direction_command(project,key) {
    if key==GUI_KEY_ESCAPE {puppy_direction_end();return puppy_title(project,puppy_line_label());}
    let mode=0;
    if key==15 {mode=1;}
    if (key==19 || key==27 || key==8) && puppy_direction_has_lines() {mode=2;if key==27 {mode=3;}if key==8 {mode=4;}}
    if key==6 && puppy_direction_has_curves(project) {mode=5;}
    if mode {puppy_direction_end();puppy_bar_page=0;puppy_direction_mode=mode;}
    return puppy_title(project,puppy_direction_hint());
}
fn puppy_direction_pick(project,x,y) {
    if puppy_direction_mode==1 {return 1;}
    let sk=j_string(puppy_draft_sketch(project),"id");let nodes=pc_nodes(cad_project_part(project));
    let point=puppy_draft_point(x,y);let center=puppy_direction_mode==5;let picked=0;
    if center {
        let hit=puppy_sketch_hit(nodes,sk,point);picked=pc_concentric_curve(nodes,sk,j_string(hit,"id"));
    }else {picked=puppy_mirror_hit(nodes,sk,point,1);}
    if !picked {return puppy_title(project,puppy_direction_hint());}
    let id=j_string(picked,"id");
    if !puppy_direction_source {puppy_direction_source=puppy_snapshot(id);return puppy_title(project,puppy_direction_hint());}
    if m_eq(id,puppy_direction_source+24) {return puppy_title(project,"Pick a different follower");}
    let label=puppy_direction_name();let kind="parallel";if puppy_direction_mode==3 {kind="perpendicular";}
    // Refreshing the report releases the picker snapshot before history is written.
    let source=m_slice(puppy_direction_source+24,load64(puppy_direction_source+16));
    let updated=0;
    if center {updated=cad_project_toggle_concentric(project,sk,source,id);}
    else if puppy_direction_mode==4 {updated=cad_project_toggle_equal_lines(project,sk,source,id);}
    else {updated=cad_project_toggle_line_direction(project,sk,source,id,kind);}
    if !updated || !puppy_line_update(project,updated) {return puppy_title(project,cg_error);}
    let report=j_parse(puppy_line_report+24);let rows=j_get(report,"lines");let i=0;
    if center {i=cg_j_count(rows);rows=j_get(report,"radial");}
    let row=pc_first(rows);
    while row && !m_eq(j_string(row,"id"),id) {i=i+1;row=load64(row+24);}
    if row {puppy_line_index=i;puppy_distance_index=-1;}
    return puppy_title(updated,m_cat(label," updated"));
}
