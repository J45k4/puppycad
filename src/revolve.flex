// The candidate is rebuilt from the saved graph. Only Apply changes the part.
global puppy_revolve_open=0;
global puppy_revolve_points=0;
global puppy_revolve_result=0;
global puppy_revolve_operation=0;
global puppy_revolve_has_body=0;
global puppy_revolve_flipped=0;

fn puppy_revolve_angle() {
    cg_begin();
    if !puppy_number_text || !m_len(puppy_number_text+24) {return 360000000;}
    let text=puppy_number_text+24;if load8(text)==43 {text=text+1;}
    return cg_number(j_new(6,text));
}

fn puppy_revolve_status(project,angle,error) {
    let operation="New body";
    if puppy_revolve_operation==1 {operation="Cut";}
    if puppy_revolve_operation==2 {operation="Join";}
    let status=m_buffer();m_text(status,"Revolve preview / ");m_text(status,operation);
    if angle>0 && angle<=360000000 {
        m_text(status," / ");m_text(status,puppy_display_number(angle));m_text(status," deg");
    }
    if puppy_revolve_flipped {m_text(status," / Flipped");}
    if error {m_text(status," / ");m_text(status,error);}
    return puppy_title(project,m_data(status));
}

fn puppy_revolve_preview(project) {
    if puppy_revolve_result {puppy_drop(puppy_revolve_result);puppy_revolve_result=0;}
    let angle=puppy_revolve_angle();
    if cg_error || angle<=0 || angle>360000000 {
        puppy_extrusion_view_show(0);
        return puppy_revolve_status(project,0,"Enter a positive angle up to 360 degrees");
    }
    let points=pc_first(j_parse(puppy_revolve_points+24));
    let a=pc_uv(points);let b=pc_uv(load64(points+24));
    if puppy_revolve_flipped {let end=a;a=b;b=end;}
    let sketch=j_string(puppy_draft_sketch(project),"id");
    let operation=puppy_extrusion_operation_name(puppy_revolve_operation);
    let result=cad_project_add_revolve(project,sketch,puppy_profile_index,a,b,angle,operation);
    if !result {
        let error=cg_error;puppy_extrusion_view_show(0);return puppy_revolve_status(project,angle,error);
    }
    let mesh=cad_project_mesh(result);
    if j_get(mesh,"error") || !puppy_extrusion_view_show(mesh) {
        puppy_extrusion_view_show(0);return puppy_revolve_status(project,angle,"Cannot display revolve");
    }
    puppy_revolve_result=puppy_snapshot(j_encode(result));return puppy_revolve_status(project,angle,0);
}

fn puppy_revolve_begin(project,a,b) {
    let stage=cad_project_extrusion_target_stage(project,"");if !stage {return puppy_title(project,cg_error);}
    let targets=cad_project_extrusion_targets(stage,"");if !targets {return puppy_title(project,cg_error);}
    if !puppy_extrusion_view_begin(stage) {return puppy_title(project,"Cannot open revolve preview");}
    let points=j_array();j_add(points,0,pc_json_point(a));j_add(points,0,pc_json_point(b));
    puppy_revolve_points=puppy_snapshot(j_encode(points));puppy_revolve_open=1;
    puppy_revolve_has_body=pc_first(targets)!=0;puppy_revolve_flipped=0;puppy_revolve_operation=0;
    if puppy_revolve_has_body {
        let operation=puppy_operation(project);
        if m_eq(operation,"cut") {puppy_revolve_operation=1;}
        if m_eq(operation,"join") {puppy_revolve_operation=2;}
    }
    puppy_revolve_axis=0;puppy_anchor=0;puppy_search_focus(0);puppy_design_focus=0;
    puppy_design_menu_close();puppy_bar_page=0;return puppy_revolve_preview(project);
}

fn puppy_revolve_close() {
    if !puppy_revolve_open {return 1;}
    if puppy_number_text {puppy_number_close();}
    if puppy_revolve_result {puppy_drop(puppy_revolve_result);puppy_revolve_result=0;}
    puppy_drop(puppy_revolve_points);puppy_revolve_points=0;
    puppy_revolve_open=0;puppy_bar_page=0;return puppy_extrusion_view_end();
}

fn puppy_revolve_apply(project) {
    if !puppy_revolve_result {return puppy_revolve_preview(project);}
    let result=cad_project_open(puppy_revolve_result+24);
    puppy_revolve_close();return puppy_finish_feature(project,result,"Revolved");
}

fn puppy_revolve_actions() {
    let rows=cg_array();puppy_compact_button(rows,"Angle",4,0);puppy_compact_button(rows,"Flip axis",9,0);
    if puppy_revolve_has_body {puppy_compact_button(rows,"Operation",18,0);}
    puppy_compact_button(rows,"Apply",40,0);puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);return rows;
}

fn puppy_revolve_key(project,key,mods) {
    if key==GUI_KEY_ESCAPE {puppy_revolve_close();return puppy_title(project,"Revolve cancelled");}
    if mods&0x3c0 {return 1;}
    if key==74 {return puppy_extrusion_view_fit();}
    if key==40 || key==88 {return puppy_revolve_apply(project);}
    if key==4 {return puppy_text_open(project,"360");}
    if key==9 {puppy_revolve_flipped=!puppy_revolve_flipped;return puppy_revolve_preview(project);}
    if key==18 && puppy_revolve_has_body {
        puppy_revolve_operation=(puppy_revolve_operation+1)%3;return puppy_revolve_preview(project);
    }
    if !(mods&3) && ((key>=30 && key<=39) || key==55 || (key>=89 && key<=99)) {
        let text=".";
        if key>=30 && key<=39 {text=m_int((key-29)%10);}
        if key>=89 && key<=98 {text=m_int((key-88)%10);}
        puppy_text_open(project,text);puppy_number_selected=0;return puppy_revolve_preview(project);
    }
    return 1;
}
