// Hold the candidate separately so evaluating a shell cannot change document history.
global puppy_shell_open=0;
global puppy_shell_editing=0;
global puppy_shell_side=0;
global puppy_shell_thickness=0;
global puppy_shell_result=0;

fn puppy_shell_face(index) {
    return cg_at(puppy_shell_faces(),index);
}

fn puppy_shell_faces() {
    let faces=cg_array();
    cg_push(faces,cg_v("z","+Z",0));cg_push(faces,cg_v("-z","−Z",0));
    cg_push(faces,cg_v("x","+X",0));cg_push(faces,cg_v("-x","−X",0));
    cg_push(faces,cg_v("y","+Y",0));cg_push(faces,cg_v("-y","−Y",0));
    return faces;
}

fn puppy_shell_status(project,error) {
    // Keep the passed diagnostic. A failed build must not block UI record creation.
    cg_begin();
    let id=puppy_shell_open+24;let message=m_buffer();
    if puppy_shell_result {m_text(message,"Shell preview ready");}
    else if puppy_shell_editing {m_text(message,"Edit shell / ");m_text(message,id);}
    else {
        let part=pc_find(j_get(project,"parts"),id);
        m_text(message,"Shell part / ");m_text(message,puppy_design_name(part,"Part"));
    }
    m_text(message," / ");m_text(message,load64(puppy_shell_face(puppy_shell_side)+8));
    m_text(message," / ");m_text(message,puppy_display_number(puppy_shell_thickness));m_text(message," mm");
    if error {m_text(message," / ");m_text(message,error);}
    return puppy_title(project,m_data(message));
}

fn puppy_shell_begin(project,part,feature) {
    let id=j_string(part,"id");let source=j_string(part,"node");let side=0;let thickness=2000000;
    if feature {
        id=j_string(feature,"id");source=id;thickness=cg_number(j_get(feature,"thickness"));
        let faces=puppy_shell_faces();
        while side<5 && !m_eq(load64(cg_at(faces,side)),j_string(feature,"open_side")) {side=side+1;}
        if !m_eq(load64(cg_at(faces,side)),j_string(feature,"open_side")) {side=0;}
        if cg_error {return puppy_title(project,cg_error);}
    }
    let mesh=pcad_feature_mesh(j_parse(j_encode(project)),source);
    if cg_error || j_get(mesh,"error") {return puppy_title(project,cg_error);}
    if !puppy_extrusion_view_begin_mesh(mesh) {return puppy_title(project,"Cannot open shell preview");}
    puppy_shell_open=puppy_snapshot(id);puppy_shell_editing=feature!=0;
    puppy_shell_side=side;puppy_shell_thickness=thickness;
    puppy_design_menu_close();puppy_search_focus(0);puppy_design_focus=0;puppy_bar_page=0;
    return puppy_shell_status(project,0);
}

fn puppy_shell_clear_result() {
    if puppy_shell_result {puppy_drop(puppy_shell_result);puppy_shell_result=0;}
    return 1;
}

fn puppy_shell_close() {
    if !puppy_shell_open {return 1;}
    if puppy_number_text {puppy_number_close();}
    puppy_shell_clear_result();puppy_drop(puppy_shell_open);puppy_shell_open=0;puppy_bar_page=0;
    return puppy_extrusion_view_end();
}

fn puppy_shell_invalidate() {
    puppy_shell_clear_result();
    return puppy_extrusion_view_show(0);
}

fn puppy_shell_evaluate(project) {
    puppy_shell_invalidate();
    let id=puppy_shell_open+24;let side=load64(puppy_shell_face(puppy_shell_side));
    let updated=0;
    if puppy_shell_editing {updated=cad_studio_set_shell(project,id,side,puppy_shell_thickness);}
    else {updated=cad_studio_add_shell(project,id,side,puppy_shell_thickness);}
    if !updated {return puppy_shell_status(project,cg_error);}
    let feature=id;
    if !puppy_shell_editing {feature=j_string(pc_find(j_get(updated,"parts"),id),"node");}
    let mesh=pcad_feature_mesh(j_parse(j_encode(updated)),feature);
    if cg_error || j_get(mesh,"error") || !puppy_extrusion_view_show(mesh) {
        puppy_extrusion_view_show(0);return puppy_shell_status(project,"Cannot display shell preview");
    }
    puppy_shell_result=puppy_snapshot(j_encode(updated));
    return puppy_shell_status(project,0);
}

fn puppy_shell_apply(project) {
    if !puppy_shell_result {return puppy_shell_status(project,"Evaluate shell first");}
    let updated=cad_project_open(puppy_shell_result+24);
    if m_eq(j_encode(updated),j_encode(project)) {
        puppy_shell_close();return puppy_title(project,"Shell unchanged");
    }
    let mesh=cad_project_mesh(updated);
    if cg_error || !puppy_upload(updated,mesh) {return puppy_shell_status(project,"Cannot display shell");}
    let label="Shell applied";if puppy_shell_editing {label="Shell updated";}
    puppy_shell_close();puppy_push(updated);
    return puppy_title(updated,label);
}

fn puppy_shell_set_thickness(project,thickness) {
    if thickness<10000 || thickness>500000000 {return puppy_title(project,"Enter a wall thickness from 0.01 to 500 mm");}
    puppy_number_close();
    if puppy_shell_thickness!=thickness {puppy_shell_thickness=thickness;puppy_shell_invalidate();}
    return puppy_shell_status(project,0);
}

fn puppy_shell_actions() {
    let rows=cg_array();let faces=puppy_shell_faces();let i=0;
    while i<cg_count(faces) {
        puppy_compact_button(rows,load64(cg_at(faces,i)+8),30+i,0);i=i+1;
    }
    puppy_compact_button(rows,"Thickness",23,0);
    puppy_compact_button(rows,"Evaluate",8,0);
    if puppy_shell_result {puppy_compact_button(rows,"Apply",40,0);}
    puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);
    return rows;
}

fn puppy_shell_key(project,key,mods) {
    if key==GUI_KEY_ESCAPE {puppy_shell_close();return puppy_title(project,"Shell cancelled");}
    if mods&0x3c3 {return 1;}
    if key>=30 && key<=35 {
        if puppy_shell_side!=key-30 {puppy_shell_side=key-30;puppy_shell_invalidate();}
        return puppy_shell_status(project,0);
    }
    if key==23 || key==59 {
        puppy_text_open(project,puppy_display_number(puppy_shell_thickness));return puppy_title(project,"Wall thickness");
    }
    if key==8 {return puppy_shell_evaluate(project);}
    if key==40 || key==88 {return puppy_shell_apply(project);}
    if key==74 {return puppy_extrusion_view_fit();}
    return 1;
}
