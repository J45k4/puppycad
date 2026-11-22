// The dialog evaluates candidate documents. Only Apply adds a history entry.
global puppy_extrusion_open=0;
global puppy_extrusion_feature=0;
global puppy_extrusion_result=0;
global puppy_extrusion_targets=0;
global puppy_extrusion_target_stage=0;
global puppy_extrusion_target_index=0;
global puppy_extrusion_picking=0;
global puppy_extrusion_length=10000000;
global puppy_extrusion_operation=0;
global puppy_extrusion_end_condition=0;
global puppy_extrusion_flip=0;

fn puppy_extrusion_operation_name(operation) {
    if operation==1 {return "cut";}
    if operation==2 {return "join";}
    return "newBody";
}
fn puppy_extrusion_end_name(condition) {
    if condition==1 {return "symmetric";}
    if condition==2 {return "through-all";}
    if condition==3 {return "up-to-body";}
    return "blind";
}
fn puppy_extrusion_target_rows() {
    return j_parse(puppy_extrusion_targets+24);
}
fn puppy_extrusion_target() {
    let row=pc_first(puppy_extrusion_target_rows());let i=0;
    while row && i<puppy_extrusion_target_index {row=load64(row+24);i=i+1;}
    return row;
}
fn puppy_extrusion_input_depth() {return puppy_extrusion_end_condition<2;}
fn puppy_extrusion_can_flip() {return puppy_extrusion_end_condition==0 || puppy_extrusion_end_condition==3;}
fn puppy_extrusion_status(project,error) {
    let operation="New body";
    if puppy_extrusion_operation==1 {operation="Cut";}
    if puppy_extrusion_operation==2 {operation="Join";}
    let condition="Blind";
    if puppy_extrusion_end_condition==1 {condition="Symmetric";}
    if puppy_extrusion_end_condition==2 {condition="Through all";}
    if puppy_extrusion_end_condition==3 {condition="Up to body";}
    let status=m_buffer();
    m_text(status,"Extrusion preview / ");m_text(status,operation);m_text(status," / ");m_text(status,condition);
    if puppy_extrusion_input_depth() {m_text(status," / ");m_text(status,puppy_display_number(puppy_extrusion_length));m_text(status," mm");}
    if puppy_extrusion_flip {m_text(status," / Flipped");}
    if puppy_extrusion_end_condition==3 {m_text(status," / Target ");m_text(status,j_string(puppy_extrusion_target(),"name"));}
    if error {m_text(status," / ");m_text(status,error);}
    return puppy_title(project,m_data(status));
}
fn puppy_extrusion_draw_result(project) {
    if puppy_extrusion_result {puppy_drop(puppy_extrusion_result);puppy_extrusion_result=0;}
    let depth=puppy_extrusion_length;let operation=puppy_extrusion_operation_name(puppy_extrusion_operation);
    let end=puppy_extrusion_end_name(puppy_extrusion_end_condition);let target="";
    if puppy_extrusion_end_condition==3 {target=j_string(puppy_extrusion_target(),"id");}
    let result=0;
    if puppy_extrusion_feature {
        result=cad_project_set_extrusion_options(project,puppy_extrusion_feature+24,depth,operation,end,puppy_extrusion_flip,target);
    }else {
        let sketch=j_string(puppy_draft_sketch(project),"id");
        result=cad_project_add_extrusion_options(project,sketch,puppy_profile_index,depth,operation,end,puppy_extrusion_flip,target);
    }
    let view=0;if result {view=cad_project_feature_history(result);}
    if !view || j_get(view,"error") {
        let error=cg_error;if !error {error="Cannot evaluate extrusion";}
        puppy_extrusion_view_show(0);return puppy_extrusion_status(project,error);
    }
    if !puppy_extrusion_view_show(j_get(view,"mesh")) {return puppy_extrusion_status(project,"Cannot display extrusion");}
    puppy_extrusion_result=puppy_snapshot(j_encode(result));return puppy_extrusion_status(project,0);
}
fn puppy_extrusion_close() {
    if !puppy_extrusion_open {return 1;}
    if puppy_extrusion_feature {puppy_drop(puppy_extrusion_feature);puppy_extrusion_feature=0;}
    if puppy_extrusion_result {puppy_drop(puppy_extrusion_result);puppy_extrusion_result=0;}
    puppy_drop(puppy_extrusion_targets);puppy_extrusion_targets=0;
    puppy_drop(puppy_extrusion_target_stage);puppy_extrusion_target_stage=0;
    puppy_extrusion_open=0;puppy_extrusion_picking=0;puppy_bar_page=0;
    puppy_extrusion_view_end();return 1;
}
fn puppy_extrusion_begin(project,feature) {
    let id="";
    if feature {
        if !m_eq(j_string(feature,"type"),"extrude") {return puppy_title(project,"Select an extrusion to edit");}
        id=j_string(feature,"id");
    }
    let targets=cad_project_extrusion_targets(project,id);if !targets {return puppy_title(project,cg_error);}
    let stage=cad_project_extrusion_target_stage(project,id);if !stage {return puppy_title(project,cg_error);}
    if !puppy_extrusion_view_begin(stage) {return puppy_title(project,"Cannot open extrusion preview");}
    puppy_search_focus(0);puppy_design_focus=0;puppy_design_menu_close();puppy_menu_rows=0;
    puppy_extrusion_open=1;puppy_bar_page=0;
    puppy_extrusion_targets=puppy_snapshot(j_encode(targets));
    puppy_extrusion_target_stage=puppy_snapshot(j_encode(stage));
    let count=cg_j_count(targets);puppy_extrusion_target_index=puppy_limit(count-1,0,count);
    puppy_extrusion_end_condition=0;puppy_extrusion_flip=0;puppy_extrusion_length=10000000;
    let operation="newBody";if !feature {operation=puppy_operation(project);}
    if feature {
        puppy_extrusion_feature=puppy_snapshot(id);operation=j_string(feature,"operation");
        puppy_extrusion_length=cg_number(j_get(feature,"depth"));
        let end=j_string(feature,"endCondition");
        if m_eq(end,"symmetric") {puppy_extrusion_end_condition=1;}
        if m_eq(end,"through-all") {puppy_extrusion_end_condition=2;}
        if m_eq(end,"up-to-body") {puppy_extrusion_end_condition=3;}
        if puppy_extrusion_can_flip() {puppy_extrusion_flip=j_value(j_get(feature,"flip"));}
        let row=pc_first(targets);let i=0;
        while row {
            if m_eq(j_string(row,"id"),j_string(feature,"targetId")) {puppy_extrusion_target_index=i;}
            row=load64(row+24);i=i+1;
        }
    }
    puppy_extrusion_operation=0;
    if m_eq(operation,"cut") {puppy_extrusion_operation=1;}
    if m_eq(operation,"join") {puppy_extrusion_operation=2;}
    if !count {puppy_extrusion_operation=0;}
    return puppy_extrusion_draw_result(project);
}
fn puppy_extrusion_apply(project) {
    if puppy_extrusion_picking || !puppy_extrusion_result {return 1;}
    let updated=cad_project_open(m_slice(puppy_extrusion_result+24,load64(puppy_extrusion_result+16)));
    let editing=puppy_extrusion_feature!=0;
    puppy_extrusion_close();
    if !editing {return puppy_finish_feature(project,updated,"Extruded");}
    if m_eq(j_encode(updated),j_encode(project)) {return puppy_title(project,"Extrusion unchanged");}
    if !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_push(updated);return puppy_title(updated,"Extrusion edited");
}
fn puppy_extrusion_set_depth(project,depth) {
    if depth<=0 || depth>500000000 {return puppy_title(project,"Enter a positive extrusion depth up to 500 mm");}
    puppy_extrusion_length=depth;puppy_number_close();return puppy_extrusion_draw_result(project);
}
fn puppy_extrusion_actions() {
    let rows=cg_array();
    if puppy_extrusion_input_depth() {puppy_compact_button(rows,"Depth",7,0);}
    puppy_compact_button(rows,"End",8,0);
    if puppy_extrusion_can_flip() {puppy_compact_button(rows,"Flip",9,0);}
    if pc_first(puppy_extrusion_target_rows()) {puppy_compact_button(rows,"Operation",18,0);}
    if puppy_extrusion_end_condition==3 {puppy_compact_button(rows,"Target",24,0);}
    puppy_compact_button(rows,"Apply",40,0);
    puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);
    return rows;
}
fn puppy_extrusion_key(project,key,mods) {
    if key==GUI_KEY_ESCAPE {
        puppy_extrusion_close();return puppy_title(project,"Extrusion cancelled");
    }
    if mods&0x3c0 {return 1;}
    if key==74 {return puppy_extrusion_view_fit();}
    if key==40 || key==88 {return puppy_extrusion_apply(project);}
    if key==7 && puppy_extrusion_input_depth() {
        return puppy_text_open(project,puppy_display_number(puppy_extrusion_length));
    }
    let count=cg_j_count(puppy_extrusion_target_rows());
    if key==8 {
        let total=2;if puppy_extrusion_operation && count {total=4;}
        puppy_extrusion_end_condition=(puppy_extrusion_end_condition+1)%total;
        if !puppy_extrusion_can_flip() {puppy_extrusion_flip=0;}
    }else if key==9 && puppy_extrusion_can_flip() {puppy_extrusion_flip=!puppy_extrusion_flip;}
    else if key==18 && count {
        puppy_extrusion_operation=(puppy_extrusion_operation+1)%3;
        if !puppy_extrusion_operation && puppy_extrusion_end_condition>=2 {puppy_extrusion_end_condition=0;puppy_extrusion_flip=0;}
    }else if key==24 && puppy_extrusion_end_condition==3 {
        puppy_extrusion_picking=1;puppy_extrusion_view_show(0);
        return puppy_title(project,"Extrusion preview / Click a target body or press Tab to cycle");
    }else if key==43 && puppy_extrusion_end_condition==3 && count {
        puppy_extrusion_target_index=(puppy_extrusion_target_index+1)%count;
    }else {return 1;}
    puppy_extrusion_picking=0;return puppy_extrusion_draw_result(project);
}
fn puppy_extrusion_event(event) {
    if !puppy_extrusion_open {return 0;}
    let kind=gui_event_kind(event);
    if kind==GUI_CLOSE || kind==GUI_QUIT {puppy_extrusion_close();return 0;}
    if puppy_extrusion_view_event(event) {return 1;}
    if kind==GUI_KEY_DOWN {return puppy_command(gui_event_code(event),gui_event_modifiers(event));}
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 {
        let x=gui_event_x(event);let y=gui_event_y(event);
        let project=cad_project_open(puppy_history+24);
        if y>=42 && y<68 {
            let width=gui_width(load64(puppy_device));
            if width<800 {return puppy_compact_bar(0,width,project,x);}
            return puppy_action_bar(puppy_extrusion_actions(),0,width,x);
        }
        if y>=76 && puppy_extrusion_picking {
            let stage=cad_project_open(puppy_extrusion_target_stage+24);
            let id=puppy_extrusion_view_pick(stage,x,y);let row=pc_first(puppy_extrusion_target_rows());let i=0;
            while row && !m_eq(id,j_string(row,"id")) {row=load64(row+24);i=i+1;}
            if !row {return puppy_title(project,"Extrusion preview / Click a valid earlier target body");}
            puppy_extrusion_target_index=i;puppy_extrusion_picking=0;return puppy_extrusion_draw_result(project);
        }
    }
    return kind!=GUI_RESIZE && kind!=GUI_EXPOSE;
}
