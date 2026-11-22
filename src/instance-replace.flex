import "cad/project-assembly.flex";

global puppy_replace_target=0;
global puppy_replace_cursor=-1;

fn puppy_replace_row(row) {
    return row && m_eq(load64(row+16),"instance") && m_eq(j_string(puppy_design_owner(row),"type"),"assembly");
}
fn puppy_replace_candidate(item,instance) {
    return m_eq(j_string(item,"type"),"part") && !m_eq(j_string(item,"id"),j_string(instance,"partId"));
}
fn puppy_replace_begin(project,instance) {
    let item=pc_first(j_get(project,"items"));
    while item && !puppy_replace_candidate(item,instance) {item=load64(item+24);}
    if !item {return puppy_title(project,"Add another part before replacing this occurrence");}
    puppy_replace_target=puppy_snapshot(j_string(instance,"id"));puppy_replace_cursor=-1;
    puppy_design_open=1;puppy_design_focus=1;puppy_search_focus(0);puppy_query_set("");
    let name=puppy_design_name(instance,"occurrence");
    return puppy_title(project,m_cat("Choose a replacement for ",name));
}
fn puppy_replace_end() {
    if puppy_replace_target {puppy_drop(puppy_replace_target);puppy_replace_target=0;}
    puppy_replace_cursor=-1;puppy_search_focus(0);puppy_query_set("");
}
fn puppy_replace_cancel(project) {
    puppy_replace_end();return puppy_title(project,"Part replacement cancelled");
}
fn puppy_replace_apply(project,part) {
    let id=m_slice(puppy_replace_target+24,load64(puppy_replace_target+16));
    let updated=cad_project_replace_instance_part(project,id,j_string(part,"id"));
    if !updated || !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_replace_end();puppy_restore_mate_pick(id,0);puppy_push(updated);
    return puppy_title(updated,m_cat("Part replaced with ",puppy_design_item_name(part,"part")));
}
fn puppy_replace_actions() {
    let rows=cg_array();puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);return rows;
}
fn puppy_replace_event(event) {
    let kind=gui_event_kind(event);
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 && gui_event_y(event)>=42 && gui_event_y(event)<68 {
        puppy_action_bar(puppy_replace_actions(),0,gui_width(load64(puppy_device)),gui_event_x(event));
    }
    return kind==GUI_MOUSE_DOWN || kind==GUI_MOUSE_UP || kind==GUI_MOUSE_MOVE || kind==GUI_WHEEL;
}
