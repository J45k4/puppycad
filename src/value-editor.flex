// A field points into the current parsed document. Only plain input text
// survives between events; an accepted edit goes through the shared CAD API.
global puppy_number_text=0;
global puppy_number_cursor=0;
global puppy_number_selected=0;
global puppy_number_paste=0;
global puppy_variable_slot=-1;

fn puppy_active_field(project) {
    if puppy_connector_rename {return cg_v(0,0,"Connector ID");}
    if puppy_connector_slot>=0 {
        return puppy_connector_field(project,puppy_design_selected(puppy_design_rows(project)),puppy_connector_slot);
    }
    if puppy_shell_open {return cg_v(0,puppy_shell_thickness,"Wall thickness  mm");}
    if puppy_revolve_open {return cg_v(0,puppy_revolve_angle(),"Revolve angle  deg");}
    if puppy_extrusion_open {return cg_v(0,puppy_extrusion_length,"Extrusion depth  mm");}
    if puppy_profile_value_id {
        let node=pc_find(pc_nodes(cad_project_part(project)),puppy_profile_value_id+24);
        return cg_v(0,0,puppy_profile_property(node));
    }
    if puppy_sheet_entry {
        let field=cg_at(puppy_sheet_setting_fields(project),puppy_sheet_setting);
        return cg_v(0,0,load64(field+8));
    }
    if puppy_patterns {return puppy_pattern_field(project);}
    if puppy_offset_pick_id {
        if !puppy_offset_editing {return cg_v(0,5000000,"New offset  mm");}
        let rel=pc_find(j_get(puppy_draft_sketch(project),"relations"),puppy_offset_pick_id+24);
        return cg_v(rel,pc_pose_number(j_get(rel,"value")),"Offset distance  mm");
    }
    if puppy_new_offset {return cg_v(0,5000000,"New offset  mm");}
    if puppy_step_path_entry {return cg_v(0,0,"STEP file path");}
    if puppy_line_report {
        if puppy_variable_slot>=0 {return cg_v(0,0,"Sketch variable");}
        let distance=puppy_distance_row();if distance {return cg_v(distance,pc_serial_number(j_get(distance,"measured"),6,1000000,1500),puppy_distance_name(distance));}
        let row=puppy_line_row();if !row {return 0;}
        if j_get(row,"field") {
            let unit="  mm";if pc_angular_field(j_string(row,"field")) {unit="  deg";}
            return cg_v(row,pc_field_number(j_get(row,"measured"),j_string(row,"field")),m_cat(puppy_curve_label(row),unit));
        }
        return cg_v(row,pc_pose_number(j_get(row,"length")),m_cat(j_string(row,"id")," / Length  mm"));
    }
    if puppy_history_rows && pc_legacy_solid_part(cad_project_part(project)) {return puppy_history_legacy_field(project);}
    if puppy_draft || puppy_empty {return 0;}
    if cad_project_is_studio(project) {
        let field=puppy_studio_field(project);if !field {return 0;}
        let value=0;if load64(field+16) {value=cg_number(load64(field+16));}
        return cg_v(field,value,m_cat(load64(field+24),m_cat("  ",load64(field+32))));
    }
    let item=cad_project_item(project);
    if m_eq(j_string(item,"type"),"assembly") {
        let data=j_get(item,"data");let servo=puppy_servo(data);
        if puppy_placement_slot>=0 && puppy_pick_id {
            let transform=cad_project_instance_transform(project,puppy_pick_id+24);if !transform {return 0;}
            let label=m_cat(puppy_placement_label(puppy_placement_slot),m_cat(" / ",puppy_placement_axis(puppy_placement_slot)));
            return cg_v(transform,puppy_placement_cell(transform,puppy_placement_slot),label);
        }
        if puppy_selected_mate {
            let mate=pc_find(j_get(data,"mates"),puppy_selected_mate+24);if !m_eq(j_string(mate,"type"),"revolute") {return 0;}
            let value=0;if servo {value=pc_pose_number(j_get(servo,"homeDeg"));}
            return cg_v(mate,value,m_cat(puppy_mate_name(mate),"  deg"));
        }
        if !servo {return 0;}
        return cg_v(servo,pc_pose_number(j_get(servo,"homeDeg")),m_cat(puppy_servo_name(data,servo),"  deg"));
    }
    let feature=puppy_feature(project);if !feature {return 0;}
    let label=m_cat(j_string(feature,"name"),m_cat(" / ",m_cat(puppy_field(feature),puppy_property_unit(feature,puppy_property))));
    return cg_v(feature,puppy_measure(project,feature),label);
}
fn puppy_value_unchanged(project,field,value) {
    let target=load64(field);
    if !value && puppy_property==1 && m_eq(j_string(target,"type"),"chamfer") && !j_get(target,"d2") {return 1;}
    if load64(field+8)!=value {return 0;}
    if puppy_selected_mate && m_eq(j_string(target,"type"),"revolute") {
        return pc_mate_servo(j_get(cad_project_item(project),"data"),puppy_selected_mate+24)!=0;
    }return 1;
}
fn puppy_write_field(project,field,value) {
    if puppy_value_unchanged(project,field,value) {return 1;}
    let target=load64(field);let updated=project;let mesh=0;
    if puppy_line_report {
        updated=cad_project_set_line_length(project,j_string(puppy_draft_sketch(project),"id"),j_string(target,"id"),value);
        return puppy_line_update(project,updated);
    }
    if puppy_history_rows && pc_legacy_solid_part(cad_project_part(project)) {
        let row=puppy_history_feature(j_parse(puppy_history_rows+24));
        updated=cad_project_edit_legacy_step(project,j_string(row,"id"),j_string(target,"id"),cg_json_number(value));
        if !updated || !puppy_refresh(updated) {return 0;}
    }else if puppy_connector_slot>=0 {
        let slot=puppy_connector_slot;
        updated=cad_project_set_connector_component(project,j_string(target,"instanceId"),j_string(target,"id"),
                                                    puppy_connector_group(slot),puppy_placement_axis(slot),value);
        if !updated || !puppy_refresh(updated) {return 0;}
    }else if cad_project_is_studio(project) {
        updated=cad_studio_set_property(project,puppy_selected,value);if !updated {return 0;}
        mesh=cad_project_mesh(updated);
    }else if m_eq(j_string(cad_project_item(project),"type"),"assembly") {
        let owner=0;let id=j_string(target,"mateId");let slot=puppy_placement_slot;
        if slot>=0 {
            owner=m_slice(puppy_pick_id+24,m_len(puppy_pick_id+24));id=0;
            updated=cad_project_set_instance_component(project,owner,puppy_placement_group(slot),puppy_placement_axis(slot),value);
        }else if puppy_selected_mate {
            owner=m_slice(puppy_pick_id+24,m_len(puppy_pick_id+24));id=j_string(target,"id");
            updated=cad_project_drive_mate(project,id,value);
        }else {updated=cad_project_servo_angle(project,id,value);}
        if !updated || !puppy_refresh(updated) {return 0;}
        if owner {puppy_restore_mate_pick(owner,id);}
        if slot>=0 {puppy_placement_open=1;puppy_placement_slot=slot;}
    }else if m_eq(j_string(target,"type"),"chamfer") {
        let first=pc_pose_number(j_get(target,"d1"));let second=pc_pose_number(j_get(target,"d2"));
        if puppy_property==0 {first=value;}else {second=value;}
        updated=cad_project_set_chamfer_distances(project,j_string(target,"id"),first,second);
        if !updated || !puppy_refresh(updated) {return 0;}
    }else {
        let number=cg_json_number(value);
        if puppy_property {
            let rect=puppy_rectangle(project,target);
            mesh=cad_project_dimension(project,j_string(rect,"id"),puppy_field(target),number);
        }else if m_eq(j_string(target,"type"),"revolve") {
            mesh=cad_project_set_revolve_angle(project,j_string(target,"id"),number);
        }else if m_eq(j_string(target,"type"),"edgeFinish") {
            mesh=cad_project_set_edge_size(project,j_string(target,"id"),number);
        }else {mesh=cad_project_set_depth(project,j_string(target,"id"),number);}
        if !mesh {return 0;}
    }
    if mesh && !puppy_upload(updated,mesh) {return 0;}
    puppy_push(updated);return 1;
}
fn puppy_number_open(project,field) {
    let text=puppy_display_number(load64(field+8));
    if puppy_line_report && j_get(load64(field),"expression") {text=j_string(load64(field),"expression");}
    return puppy_text_open(project,text);
}
fn puppy_text_open(project,text) {
    puppy_search_focus(0);puppy_copy_focus=0;
    puppy_number_text=puppy_snapshot(text);
    puppy_number_cursor=m_len(puppy_number_text+24);puppy_number_selected=1;
    gui_text_focus(load64(puppy_device),2);
    return puppy_title(project,"Set value");
}
fn puppy_variable_row(project) {
    let row=pc_first(j_get(puppy_draft_sketch(project),"variables"));let i=0;
    while row && i<puppy_variable_slot {row=load64(row+24);i=i+1;}return row;
}
fn puppy_variable_hint(project) {
    if puppy_variable_row(project) {return "Edit name = expression   |   Tab: next or new   |   Clear to delete";}
    return "New variable   |   Enter name = expression   |   Tab: next";
}
fn puppy_variable_open(project,slot) {
    let count=cg_j_count(j_get(puppy_draft_sketch(project),"variables"));puppy_variable_slot=slot%(count+1);
    let row=puppy_variable_row(project);let text="";
    if row {text=m_cat(j_string(row,"name"),m_cat(" = ",j_string(row,"expression")));}
    if puppy_number_text {puppy_drop(puppy_number_text);}
    puppy_number_text=puppy_snapshot(text);puppy_number_selected=1;puppy_number_cursor=m_len(text);
    gui_text_focus(load64(puppy_device),2);return puppy_title(project,puppy_variable_hint(project));
}
fn puppy_variable_apply(project,text) {
    let name=j_string(puppy_variable_row(project),"name");let next=name;let i=0;
    while load8(text+i) && load8(text+i)!=61 {i=i+1;}
    if load8(text+i) {next=m_slice(text,i);text=text+i+1;}
    else if !m_len(name) {return puppy_title(project,"Enter name = expression");}
    let updated=cad_project_edit_line_variable(project,j_string(puppy_draft_sketch(project),"id"),name,next,text);
    if !updated || !puppy_line_update(project,updated) {return puppy_title(project,cg_error);}
    puppy_number_close();return puppy_title(updated,"Variables updated");
}
fn puppy_number_close() {
    if puppy_profile_value_id {puppy_drop(puppy_profile_value_id);puppy_profile_value_id=0;}
    puppy_sheet_entry=0;
    puppy_new_offset=0;
    if puppy_offset_pick_id {puppy_drop(puppy_offset_pick_id);puppy_offset_pick_id=0;}
    puppy_placement_slot=-1;
    puppy_connector_slot=-1;
    puppy_connector_rename=0;
    puppy_step_path_entry=0;
    if puppy_number_paste {gui_clipboard_cancel(puppy_number_paste);puppy_number_paste=0;}
    puppy_variable_slot=-1;puppy_drop(puppy_number_text);puppy_number_text=0;puppy_toolbar_dirty=1;
    return gui_text_focus(load64(puppy_device),0);
}
fn puppy_number_apply(project) {
    let text=puppy_number_text+24;
    if puppy_connector_rename {return puppy_connector_rename_apply(project,text);}
    if puppy_revolve_open {return puppy_revolve_apply(project);}
    if puppy_profile_value_id {return puppy_profile_apply(project,text);}
    if puppy_sheet_entry {return puppy_sheet_entry_apply(project,text);}
    if puppy_patterns {return puppy_pattern_apply(project,text);}
    if puppy_step_path_entry {return puppy_step_start(project,text);}
    if puppy_offset_pick_id {
        let sk=j_string(puppy_draft_sketch(project),"id");let id=puppy_offset_pick_id+24;let updated=0;
        if puppy_offset_editing {updated=cad_project_set_offset_distance(project,sk,id,text);}
        else {updated=cad_project_add_offset(project,sk,id,text);}
        if !updated {return puppy_title(project,cg_error);}
        if !m_eq(j_encode(project),j_encode(updated)) {puppy_push(updated);}
        puppy_fit_sketch(updated,1);puppy_number_close();return puppy_title(updated,"Offset applied");
    }
    if puppy_variable_slot>=0 {return puppy_variable_apply(project,text);}
    if puppy_line_report {
        let id=j_string(puppy_draft_sketch(project),"id");let distance=puppy_distance_row();let updated=0;
        let row=puppy_line_row();
        if puppy_new_offset {updated=cad_project_add_offset(project,id,j_string(row,"id"),text);}
        else if distance {updated=cad_project_set_distance_expression(project,id,j_string(distance,"id"),text);}
        else if po_kind(row) {updated=cad_project_set_offset_distance(project,id,j_string(row,"id"),text);}
        else if j_get(row,"field") {updated=cad_project_set_curve_dimension(project,id,j_string(row,"id"),j_string(row,"field"),text);}
        else {updated=cad_project_set_line_expression(project,id,j_string(row,"id"),text);}
        if !updated {return puppy_title(project,cg_error);}
        if m_eq(j_encode(updated),j_encode(project)) {puppy_number_close();return puppy_title(project,"Value unchanged");}
        if !puppy_line_update(project,updated) {return puppy_title(project,cg_error);}
        if puppy_new_offset {
            let report=j_parse(puppy_line_report+24);
            puppy_line_index=cg_j_count(j_get(report,"lines"))+cg_j_count(j_get(report,"radial"))+cg_j_count(j_get(report,"offsets"))-1;
            puppy_fit_sketch(updated,1);puppy_number_close();return puppy_title(updated,puppy_line_label());
        }
        puppy_number_close();return puppy_title(updated,"Edited");
    }
    let digits=text;
    if load8(text)==43 || load8(text)==45 {digits=digits+1;}
    if load8(digits)==46 {digits=digits+1;}
    if load8(digits)<48 || load8(digits)>57 {return puppy_title(project,"Enter a signed decimal number");}
    if load8(text)==43 {text=text+1;}
    let limit=500;
    let history_field=puppy_history_legacy_field(project);
    if history_field && m_eq(j_string(load64(history_field),"id"),"segments") {limit=1024;}
    cg_begin();let value=cg_number_bounded(j_new(6,text),limit);
    if cg_error {
        if limit==1024 {return puppy_title(project,"Enter a whole number of segments from 8 to 1024");}
        return puppy_title(project,"Enter a decimal from -500 to 500 with up to six places");
    }
    if puppy_shell_open {return puppy_shell_set_thickness(project,value);}
    if puppy_extrusion_open {return puppy_extrusion_set_depth(project,value);}
    let field=puppy_active_field(project);
    if field && puppy_value_unchanged(project,field,value) {
        puppy_number_close();return puppy_title(project,"Value unchanged");
    }
    if !field || !puppy_write_field(project,field,value) {
        let error=cg_error;if !error {error="Could not rebuild the drawing";}
        return puppy_title(cad_project_open(puppy_history+24),error);
    }
    let updated=cad_project_open(puppy_history+24);let message="Edited";
    if history_field {
        puppy_number_close();return puppy_history_select(updated,j_parse(puppy_history_rows+24));
    }
    if puppy_placement_slot>=0 {
        let solved=puppy_active_field(updated);
        if solved && load64(solved+8)!=value {message=m_cat("Mate solved to ",puppy_display_number(load64(solved+8)));}
    }
    puppy_number_close();return puppy_title(updated,message);
}
fn puppy_number_replace(text,remove_left,remove_right) {
    let old=puppy_number_text+24;let n=m_len(old);let at=puppy_number_cursor;
    let end=puppy_limit(at+remove_right,0,n);at=puppy_limit(at-remove_left,0,n);
    while at>0 && (load8(old+at)&192)==128 {at=at-1;}
    while end<n && (load8(old+end)&192)==128 {end=end+1;}
    if puppy_number_selected {at=0;end=n;}
    let length=m_len(text);let i=0;
    let limit=512;if puppy_variable_slot>=0 {limit=580;}
    if puppy_step_path_entry && length+n-(end-at)>4095 {return puppy_title(cad_project_open(puppy_history+24),"STEP path is too long");}
    if puppy_line_report && length+n-(end-at)>limit {return puppy_title(cad_project_open(puppy_history+24),"Expression entry is too long");}
    if puppy_sheet_entry && length+n-(end-at)>256 {return puppy_title(cad_project_open(puppy_history+24),"Drawing field is too long");}
    while i<length {
        let c=load8(text+i);
        if (puppy_shell_open || puppy_revolve_open) && (c<48 || c>57) && c!=46 && c!=43 && c!=45 {return 1;}
        if c<32 || c==127 || (!puppy_sheet_entry && !puppy_step_path_entry && !puppy_line_report && (c>126 || length+n-(end-at)>40)) {
            return puppy_title(cad_project_open(puppy_history+24),"Enter a decimal number of up to 40 characters");
        }
        i=i+1;
    }
    let next=puppy_snapshot(m_cat(m_slice(old,at),m_cat(text,old+end)));
    puppy_drop(puppy_number_text);puppy_number_text=next;puppy_number_cursor=at+length;
    puppy_number_selected=0;let project=cad_project_open(puppy_history+24);
    if puppy_shell_open {return puppy_title(project,"Wall thickness");}
    if puppy_revolve_open {return puppy_revolve_preview(project);}
    if puppy_step_path_entry {return puppy_title(project,"Open STEP file   |   Enter a path");}
    if puppy_variable_slot>=0 {return puppy_title(project,puppy_variable_hint(project));}
    return puppy_title(project,"Set value");
}
fn puppy_number_event(event) {
    if !puppy_number_text {return 0;}
    let kind=gui_event_kind(event);let key=gui_event_code(event);
    if kind==GUI_CLOSE || kind==GUI_QUIT {puppy_number_close();return 0;}
    if (puppy_revolve_open || puppy_shell_open) && puppy_extrusion_view_event(event) {return 1;}
    if kind==GUI_TEXT {
        return puppy_number_replace(gui_event_text(event),0,0);
    }
    if kind==GUI_IME {
        let data=gui_ime_data(key);
        if data && load64(data+64) && !load64(data+80) {puppy_number_replace(load64(data+96),0,0);}
        return 1;
    }
    if kind==GUI_CLIPBOARD && key==puppy_number_paste {
        let text=gui_clipboard_take(key);puppy_number_paste=0;
        if text {puppy_number_replace(text,0,0);gui_clipboard_release(text);}return 1;
    }
    if kind==GUI_MOUSE_DOWN {
        let w=gui_width(load64(puppy_device));let x=gui_event_x(event);let y=gui_event_y(event);
        if gui_event_button(event)!=1 {return 1;}
        if puppy_breadcrumb_target(x,y) {return puppy_breadcrumb_click(cad_project_open(puppy_history+24),x,y);}
        if y>=42 && y<68 {
            if x>=w-150 && x<w-86 {return puppy_number_apply(cad_project_open(puppy_history+24));}
            if x>=w-78 && x<w-8 {
                if puppy_revolve_open {return puppy_revolve_key(cad_project_open(puppy_history+24),GUI_KEY_ESCAPE,0);}
                puppy_number_close();let project=cad_project_open(puppy_history+24);
                if puppy_shell_open {return puppy_shell_status(project,0);}
                if puppy_extrusion_open {return puppy_extrusion_status(project,0);}
                return puppy_title(project,"Edit cancelled");
            }
        }
        return 1;
    }
    if kind!=GUI_KEY_DOWN {return kind!=GUI_RESIZE && kind!=GUI_EXPOSE;}
    if puppy_revolve_open && (key==GUI_KEY_ESCAPE || ((key==9 || key==18) && !(gui_event_modifiers(event)&0x3c3))) {
        return puppy_revolve_key(cad_project_open(puppy_history+24),key,0);
    }
    let n=m_len(puppy_number_text+24);puppy_toolbar_dirty=1;
    if gui_event_modifiers(event)&192 {
        if key==4 {puppy_number_selected=1;puppy_number_cursor=n;}
        if key==25 {puppy_number_paste=gui_clipboard_request(load64(puppy_device));}
        if key==6 && puppy_number_selected {gui_clipboard_copy(puppy_number_text+24);}
        return 1;
    }
    if key==GUI_KEY_ESCAPE {
        puppy_number_close();let project=cad_project_open(puppy_history+24);
        if puppy_shell_open {return puppy_shell_status(project,0);}
        if puppy_extrusion_open {return puppy_extrusion_status(project,0);}
        return puppy_title(project,"Edit cancelled");
    }
    if key==43 && puppy_variable_slot>=0 {return puppy_variable_open(cad_project_open(puppy_history+24),puppy_variable_slot+1);}
    if key==40 || key==88 {return puppy_number_apply(cad_project_open(puppy_history+24));}
    if key==42 || key==76 {return puppy_number_replace("",key==42,key==76);}
    if key==74 || key==77 || key==GUI_KEY_LEFT || key==GUI_KEY_RIGHT {
        if key==74 || (key==GUI_KEY_LEFT && puppy_number_selected) {puppy_number_cursor=0;}
        else if key==77 || (key==GUI_KEY_RIGHT && puppy_number_selected) {puppy_number_cursor=n;}
        else if key==GUI_KEY_LEFT {
            puppy_number_cursor=puppy_limit(puppy_number_cursor-1,0,n);
            while puppy_number_cursor>0 && (load8(puppy_number_text+24+puppy_number_cursor)&192)==128 {puppy_number_cursor=puppy_number_cursor-1;}
        }
        else {
            puppy_number_cursor=puppy_limit(puppy_number_cursor+1,0,n);
            while puppy_number_cursor<n && (load8(puppy_number_text+24+puppy_number_cursor)&192)==128 {puppy_number_cursor=puppy_number_cursor+1;}
        }
        puppy_number_selected=0;
    }
    return 1;
}
fn puppy_number_bar(rgb,w,project,breadcrumbs) {
    let field=puppy_active_field(project);if !field {return 0;}
    let label=load64(field+16);
    if puppy_status && !m_eq(puppy_status+24,"Set value") {label=puppy_status+24;}
    let x=12;let part=cad_project_part(project);
    if breadcrumbs && part && (puppy_sketch || puppy_workspace_parent(project,part)) {
        let path_edge=puppy_limit(w-20-puppy_toolbar_text_width(label),w/2,w-8);
        x=puppy_toolbar_breadcrumbs(project,rgb,w,x,path_edge)+8;
    }
    puppy_bar_text(rgb,w,x,25,label,w-8);
    let end=w-158;puppy_bar_rect(rgb,w,12,42,end-12,26,20,30,34);
    let text=puppy_number_text+24;
    if puppy_number_selected {puppy_bar_rect(rgb,w,17,44,puppy_limit(m_len(text)*10+6,0,end-17),22,54,85,82);}
    else {
        let start=puppy_limit(puppy_number_cursor-(end-38)/10,0,puppy_number_cursor);
        while start>0 && (load8(text+start)&192)==128 {start=start-1;}
        text=m_cat(m_slice(text+start,puppy_number_cursor-start),m_cat("│",text+puppy_number_cursor));
    }
    puppy_bar_text(rgb,w,20,60,text,end-2);
    puppy_bar_button(rgb,w,w-150,64,"Apply");puppy_bar_button(rgb,w,w-78,70,"Cancel");return 1;
}
