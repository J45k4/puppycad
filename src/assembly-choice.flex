import "cad/project-assembly.flex";

global puppy_assembly_choice_kind=0;
global puppy_assembly_choice_target=0;
global puppy_assembly_choice_cursor=-1;

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
    let name=puppy_design_name(instance,"occurrence");
    return puppy_assembly_choice_open(project,1,instance,m_cat("Choose a replacement for ",name));
}
fn puppy_assembly_choice_open(project,kind,target,message) {
    puppy_assembly_choice_kind=kind;puppy_assembly_choice_target=puppy_snapshot(j_encode(target));
    puppy_design_open=1;puppy_design_focus=1;puppy_search_focus(0);puppy_query_set("");
    return puppy_title(project,message);
}
fn puppy_assembly_choice_end() {
    puppy_drop(puppy_assembly_choice_target);puppy_assembly_choice_target=0;puppy_assembly_choice_kind=0;
    puppy_search_focus(0);puppy_query_set("");
}
fn puppy_assembly_choice_cancel(project) {
    let message="Part replacement cancelled";
    if puppy_assembly_choice_kind==2 {message="Connector attachment cancelled";}
    puppy_assembly_choice_end();return puppy_title(project,message);
}
fn puppy_assembly_choice_title() {
    if puppy_assembly_choice_kind==2 {return "Attach connector";}return "Replace part";
}
fn puppy_assembly_choice_search() {
    if puppy_assembly_choice_kind==2 {return "Search occurrences";}return "Search parts";
}
fn puppy_assembly_choice_rows(rows,project) {
    let target=j_parse(puppy_assembly_choice_target+24);let candidate=0;
    if puppy_assembly_choice_kind==1 {
        candidate=pc_first(j_get(project,"items"));
        while candidate {
            if puppy_replace_candidate(candidate,target) {
                puppy_design_row(rows,0,"part",candidate,puppy_design_item_name(candidate,"part"),0);
            }
            candidate=load64(candidate+24);
        }
    }else {
        let owner=j_string(target,"instanceId");
        if m_len(owner) {puppy_design_row(rows,0,"world",0,"World",0);}
        candidate=pc_first(j_get(j_get(cad_project_item(project),"data"),"instances"));
        while candidate {
            if !m_eq(j_string(candidate,"id"),owner) {
                puppy_design_row(rows,0,"instance",candidate,puppy_design_name(candidate,"Occurrence"),0);
            }
            candidate=load64(candidate+24);
        }
    }
    return 1;
}
fn puppy_assembly_choice_apply(project,candidate) {
    let target=j_parse(puppy_assembly_choice_target+24);
    let target_id=j_string(target,"id");let id=m_slice(target_id,m_len(target_id));
    if puppy_assembly_choice_kind==2 {
        let owner=j_string(candidate,"id");
        let updated=puppy_connector_reassign(project,target,owner,id);
        if !updated {return puppy_title(project,cg_error);}
        puppy_assembly_choice_end();
        return puppy_title(updated,m_cat("Connector attached to ",puppy_design_name(candidate,"World")));
    }
    let updated=cad_project_replace_instance_part(project,id,j_string(candidate,"id"));
    if !updated || !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_assembly_choice_end();puppy_restore_mate_pick(id,0);puppy_push(updated);
    return puppy_title(updated,m_cat("Part replaced with ",puppy_design_item_name(candidate,"part")));
}
fn puppy_assembly_choice_actions() {
    let rows=cg_array();puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);return rows;
}
fn puppy_assembly_choice_event(event) {
    let kind=gui_event_kind(event);
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 && gui_event_y(event)>=42 && gui_event_y(event)<68 {
        puppy_action_bar(puppy_assembly_choice_actions(),0,gui_width(load64(puppy_device)),gui_event_x(event));
    }
    return kind==GUI_MOUSE_DOWN || kind==GUI_MOUSE_UP || kind==GUI_MOUSE_MOVE || kind==GUI_WHEEL;
}
