import "workspaces.flex";
import "viewport.flex";
import "value-editor.flex";
import "edge-finish.flex";
import "feature-history.flex";
import "read-only.flex";
import "line-editor.flex";
import "pattern.flex";
import "step-import.flex";
import "mate-editor.flex";
import "extrusion.flex";
import "revolve.flex";

// History owns immutable document snapshots. Temporary evaluation and drawing
// allocations belong to the event's arena and are discarded after each event.
global puppy_history=0;
global puppy_root=0;
global puppy_disk=0;
global puppy_revision=0;
global puppy_path=0;
global puppy_selected=0;
global puppy_property=0;
global puppy_saved_history=0;
global puppy_exit_requested=0;
global puppy_part_choice=0;
global puppy_picking_face=0;
global puppy_pending_title=0;
global puppy_projection_save_needed=0;

fn puppy_snapshot(text) {
    let n=m_len(text);let snapshot=alloc(n+25);
    m_assert(snapshot>0,"Cannot allocate document history");
    store64(snapshot,0);store64(snapshot+8,0);store64(snapshot+16,n);
    m_copy(snapshot+24,text,n+1);return snapshot;
}
fn puppy_drop(snapshot) {return syscall(11,snapshot,load64(snapshot+16)+25,0,0,0,0);}
fn puppy_trim(snapshot) {
    while snapshot {
        let next=load64(snapshot+8);if puppy_saved_history==snapshot {puppy_saved_history=0;}puppy_drop(snapshot);snapshot=next;
    }
    return 0;
}
fn puppy_push(project) {
    let rebuilt=cad_project_rebuild_projections(project);
    if rebuilt {project=rebuilt;}
    let next=puppy_snapshot(j_encode(project));
    puppy_trim(load64(puppy_history+8));store64(puppy_history+8,next);
    store64(next,puppy_history);puppy_history=next;puppy_part_sketch_dirty=1;return 1;
}
fn puppy_driving_feature(row) {
    let kind=j_string(row,"type");
    return (m_eq(kind,"extrude") || m_eq(kind,"revolve") || m_eq(kind,"edgeFinish")) && !j_value(j_get(row,"suppressed"));
}
fn puppy_feature(project) {
    let part=cad_project_part(project);if !part {return 0;}
    let row=pc_first(pc_nodes(part));let first=0;let i=0;let position=0;
    let bar=j_get(j_get(j_get(part,"data"),"tree"),"rollbackIndex");
    while row && (!bar || position<j_value(bar)) {
        if pc_model_feature(row) {position=position+1;}
        if puppy_driving_feature(row) {
            if i==puppy_selected {return row;}if !first {first=row;}i=i+1;
        }
        row=load64(row+24);
    }
    puppy_selected=0;return first;
}
fn puppy_property_name(feature,property) {
    if property==1 {return "width";}
    if property==2 {return "height";}
    if m_eq(j_string(feature,"type"),"revolve") {return "angle";}
    if m_eq(j_string(feature,"type"),"edgeFinish") {return pc_edge_field(feature);}
    return "depth";
}
fn puppy_field(feature) {return puppy_property_name(feature,puppy_property);}
fn puppy_property_unit(feature,property) {
    if !property && m_eq(j_string(feature,"type"),"revolve") {return " deg";}
    return " mm";
}
fn puppy_profile(project,feature) {
    let nodes=pc_nodes(cad_project_part(project));
    return cad_project_profile(nodes,pc_find(nodes,j_string(feature,"sketchId")),feature);
}
fn puppy_servo(data) {
    if puppy_selected_mate {return pc_mate_servo(data,puppy_selected_mate+24);}
    let row=pc_first(j_get(data,"actuators"));let first=0;let i=0;
    while row {
        if m_eq(j_string(row,"type"),"servo") && j_get(row,"homeDeg") {
            if i==puppy_selected {return row;}if !first {first=row;}i=i+1;
        }
        row=load64(row+24);
    }
    puppy_selected=0;return first;
}
fn puppy_servo_name(data,servo) {
    let mate=pc_find(j_get(data,"mates"),j_string(servo,"mateId"));
    let name=j_string(mate,"name");if !m_len(name) {name=j_string(servo,"mateId");}
    return name;
}
fn puppy_servo_label(data,servo) {
    return m_cat(puppy_servo_name(data,servo),m_cat("  ",m_cat(puppy_display_number(pc_pose_number(j_get(servo,"homeDeg")))," deg")));
}
fn puppy_rectangle(project,feature) {
    if !j_get(feature,"sketchId") {return 0;}
    let region=puppy_profile(project,feature);
    if !region {return 0;}
    let entity=load64(cg_at(region,0)+8);
    if m_eq(j_string(entity,"type"),"sketchCornerRectangle") {return entity;}
    return 0;
}
fn puppy_measure(project,feature) {
    if !puppy_property {return pc_pose_number(j_get(feature,puppy_field(feature)));}
    return cad_rectangle_dimension(puppy_rectangle(project,feature),puppy_field(feature));
}
fn puppy_display_dimension(project,feature) {
    return puppy_display_number(puppy_measure(project,feature));
}
fn puppy_display_number(n) {
    if n%1000000==0 {return cg_int(n/1000000);}
    let number=cg_decimal(n);let length=m_len(number);
    while load8(number+length-1)==48 {length=length-1;}
    return m_slice(number,length);
}
fn puppy_studio_field(project) {
    let fields=cad_studio_properties(project);if puppy_selected>=cg_count(fields) {puppy_selected=0;}
    return cg_at(fields,puppy_selected);
}
fn puppy_studio_label(project) {
    let field=puppy_studio_field(project);if !field {return "PCAD design";}
    let value=0;if load64(field+16) {value=cg_number(load64(field+16));}
    return m_cat(load64(field+24),m_cat("  ",m_cat(puppy_display_number(value),m_cat(" ",load64(field+32)))));
}
fn puppy_document_name(project) {
    if !cad_project_is_studio(project) {return j_string(cad_project_item(project),"name");}
    let name=j_string(project,"name");if m_len(name) {return name;}
    let base=puppy_path;let i=0;while load8(puppy_path+i) {if load8(puppy_path+i)==47 {base=puppy_path+i+1;}i=i+1;}return base;
}
fn puppy_title(project,message) {
    if puppy_status {puppy_drop(puppy_status);puppy_status=0;}
    if message {puppy_status=puppy_snapshot(message);}
    puppy_toolbar_dirty=1;puppy_tree_dirty=1;
    let feature=puppy_feature(project);let title=m_buffer();m_text(title,"PuppyCAD  |  ");
    let item=cad_project_item(project);m_text(title,puppy_document_name(project));
    if m_eq(j_string(item,"type"),"assembly") || (cad_project_is_studio(project) && j_get(project,"assembly")) {
        m_text(title,"  |  Assembly");let data=j_get(item,"data");let servo=0;if data {servo=puppy_servo(data);}
        if servo {m_text(title,"  |  ");m_text(title,puppy_servo_label(data,servo));}
    }else if puppy_sketch {m_text(title,"  |  Sketch");}else {m_text(title,"  |  Solid");}
    if cad_project_is_studio(project) {m_text(title,"  |  ");m_text(title,puppy_studio_label(project));}
    else if puppy_draft {
        m_text(title,"  |  ");m_text(title,j_string(puppy_draft_sketch(project),"name"));
    }else if feature {
        m_text(title,"  |  ");m_text(title,j_string(feature,"name"));m_text(title,"  ");
        m_text(title,puppy_field(feature));m_text(title," ");m_text(title,puppy_display_dimension(project,feature));m_text(title,puppy_property_unit(feature,puppy_property));
    }
    if message {m_text(title,"  |  ");m_text(title,message);}
    if puppy_pending_title {puppy_drop(puppy_pending_title);}
    puppy_pending_title=puppy_snapshot(m_data(title));return 1;
}
fn puppy_save(project) {
    if puppy_disk && puppy_saved_history==puppy_history && !puppy_projection_save_needed {return puppy_title(project,"Saved");}
    if puppy_disk && !m_eq(puppy_read(puppy_path),puppy_disk+24) {return puppy_title(project,"File changed on disk");}
    if !cad_project_is_studio(project) && puppy_revision==0x7fffffffffffffff {return puppy_title(project,"Revision limit reached");}
    if !cad_project_is_studio(project) {cg_j_int(project,"revision",puppy_revision+1);}
    let text=j_encode(project);
    let saved=0;
    if puppy_disk {saved=puppy_write(puppy_path,text,m_len(text));}
    else {
        let staged=puppy_stage(puppy_path,text,m_len(text));
        if staged {
            let status=syscall(316,-100,staged,-100,puppy_path,1,0);
            if !status {saved=1;}else {syscall(87,staged,0,0,0,0,0);}
            if status==-17 {return puppy_title(project,"File changed on disk");}
        }
    }
    if !saved {return puppy_title(project,"Save failed");}
    if puppy_disk {puppy_drop(puppy_disk);}puppy_disk=puppy_snapshot(text);puppy_revision=puppy_revision+1;puppy_saved_history=puppy_history;
    puppy_projection_save_needed=0;
    return puppy_title(project,"Saved");
}
fn puppy_has_other_item(project) {
    let row=pc_first(j_get(project,"items"));let count=0;
    while row && count<2 {
        let kind=j_string(row,"type");if m_eq(kind,"part") || m_eq(kind,"assembly") {count=count+1;}
        row=load64(row+24);
    }
    return count>1;
}
fn puppy_next_item(project) {
    if !puppy_has_other_item(project) {return 1;}
    let active=cad_project_item(project);let row=pc_first(j_get(project,"items"));let index=0;
    let first=-1;let next=-1;let passed=0;
    while row && next<0 {
        let kind=j_string(row,"type");
        if m_eq(kind,"part") || m_eq(kind,"assembly") {
            if first<0 {first=index;}if passed {next=index;}
            if row==active {passed=1;}
        }
        row=load64(row+24);index=index+1;
    }
    if next<0 {next=first;}
    if !puppy_open_item(project,next) {return puppy_title(cad_project_open(puppy_history+24),"Cannot open this item");}return 1;
}
fn puppy_item_index(project,item) {
    let row=pc_first(j_get(project,"items"));let index=0;
    while row && row!=item {row=load64(row+24);index=index+1;}
    if row {return index;}return -1;
}
fn puppy_open_item(project,next) {
    if next<0 {return 0;}
    let previous_item=cad_project_item(project);
    let selection=j_array();j_add(selection,0,j_number(next));cg_j_set(project,"selectedPath",selection);
    let previous=puppy_draft_index;puppy_draft_index=0;
    if !puppy_refresh(project) {puppy_draft_index=previous;return 0;}
    if previous_item!=cad_project_item(project) {puppy_workspace_track_parent(project,previous_item,cad_project_part(project));}
    puppy_selected=0;puppy_property=0;puppy_anchor=0;puppy_revolve_axis=0;puppy_part_choice=0;puppy_sketch=puppy_draft;
    puppy_push(project);return puppy_title(project,"Item opened");
}
fn puppy_insert_part_choice(project) {
    let row=pc_first(j_get(project,"items"));let first=0;let index=0;
    while row {
        if m_eq(j_string(row,"type"),"part") {
            if !first {first=row;}
            if index==puppy_part_choice {return row;}index=index+1;
        }
        row=load64(row+24);
    }
    puppy_part_choice=0;return first;
}
fn puppy_add_item(project,kind) {
    let updated=cad_project_add_item(project,kind);
    if !updated || !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_workspace_clear_parent();
    puppy_part_choice=0;puppy_selected=0;puppy_property=0;puppy_anchor=0;puppy_revolve_axis=0;puppy_sketch=puppy_draft;
    puppy_push(updated);
    if puppy_draft {return puppy_title(updated,puppy_draft_hint());}
    return puppy_title(updated,"Choose a part to insert");
}
fn puppy_export_shortcut(project,extension) {
    let base=puppy_path;let n=m_len(base);
    if n>=5 && m_eq(base+n-5,".pcad") {base=m_slice(base,n-5);}
    let out=puppy_export(project,m_cat(base,extension),1);
    if !out {return puppy_title(project,cg_error);}
    return puppy_title(project,m_cat("Exported ",out));
}
fn puppy_command(key,mods) {
    puppy_forget_click();
    if puppy_revolve_open {return puppy_revolve_key(cad_project_open(puppy_history+24),key,mods);}
    if puppy_extrusion_open {return puppy_extrusion_key(cad_project_open(puppy_history+24),key,mods);}
    if key==60 && !(mods&0x3c3) && (puppy_design_available() || puppy_tree_open || puppy_step_input) {return puppy_design_toggle();}
    if puppy_plane_chooser {return puppy_plane_command(cad_project_open(puppy_history+24),key,mods);}
    if puppy_sheet_scene {return puppy_sheet_key(cad_project_open(puppy_history+24),key,mods);}
    if puppy_join_drag {
        if key==GUI_KEY_ESCAPE {puppy_join_end_drag();return puppy_title(cad_project_open(puppy_history+24),"Smooth drag cancelled");}return 1;
    }
    if puppy_construction_ids {return puppy_construction_key(cad_project_open(puppy_history+24),key);}
    if puppy_mating {return puppy_mate_key(cad_project_open(puppy_history+24),key);}
    if puppy_step_input {return puppy_step_key(cad_project_open(puppy_history+24),key);}
    if puppy_patterns {
        if puppy_pattern_drag {if key==GUI_KEY_ESCAPE {return puppy_pattern_cancel(cad_project_open(puppy_history+24));}return 1;}
        if !(mods&0x3c0) {return puppy_pattern_key(cad_project_open(puppy_history+24),key);}
        if !(mods&192) || (key!=29 && key!=28 && key!=22 && key!=8) {return 1;}
    }else if key==62 && !(mods&0x3c0) && puppy_draft && !puppy_line_report && !puppy_loft_open && !puppy_revolve_axis {
        return puppy_pattern_begin(cad_project_open(puppy_history+24));
    }
    if key==12 && mods&192 && !puppy_line_report && !puppy_loft_open && !puppy_history_rows && !puppy_edge_kind {
        return puppy_step_prompt(cad_project_open(puppy_history+24));
    }
    if puppy_line_report {
        if !(mods&0x3c0) {return puppy_line_command(cad_project_open(puppy_history+24),key);}
        if puppy_line_drag || !(mods&192) || (key!=29 && key!=28 && key!=22 && key!=8) {return 1;}
    }
    if key==65 && !(mods&0x3c0) && !puppy_history_rows && !puppy_edge_kind && !puppy_loft_open {
        let project=cad_project_open(puppy_history+24);if cad_project_part(project) {return puppy_line_choose(project,0);}
    }
    if puppy_history_rows {
        if !(mods&0x3c0) {return puppy_history_key(cad_project_open(puppy_history+24),key);}
        if !(mods&192) || (key!=29 && key!=28 && key!=22 && key!=8) {return 1;}
    }
    if !puppy_history_rows && key==64 && !puppy_edge_kind && !puppy_loft_open {
        let current=cad_project_open(puppy_history+24);
        if cad_project_part(current) {return puppy_history_begin(current);}
    }
    if puppy_edge_kind {return puppy_edge_key(cad_project_open(puppy_history+24),key,mods);}
    if puppy_loft_open {return puppy_loft_key(cad_project_open(puppy_history+24),key,mods);}
    if key==11 && !(mods&0x3c0) && !puppy_sketch && !puppy_empty {
        let project=cad_project_open(puppy_history+24);
        if cad_project_part(project) {return puppy_edge_begin(project,mods&3);}
    }
    if key==38 && !(mods&0x3c0) {
        let project=cad_project_open(puppy_history+24);if cad_project_part(project) {return puppy_loft_begin(project);}
    }
    if puppy_button {
        puppy_view_end(0);if key==GUI_KEY_ESCAPE {return 1;}
    }
    if key==23 && !(mods&0x3c0) && !puppy_sketch && !puppy_empty {return puppy_objects_toggle();}
    if key==68 && puppy_isolated && !(mods&0x3c0) {
        return puppy_isolate(cad_project_open(puppy_history+24),0);
    }
    if puppy_camera_available() && !(mods&0x3c0) {
        if key==18 {
            puppy_orthographic=!puppy_orthographic;
            let label="Perspective";if puppy_orthographic {label="Orthographic";}
            return puppy_title(cad_project_open(puppy_history+24),label);
        }
        if key>=30 && key<=35 {return puppy_standard_view(key-30);}
        if key==39 {return puppy_standard_view(6);}
    }
    if key==74 && puppy_camera_available() {return puppy_fit();}
    if key==GUI_KEY_UP_ARROW {puppy_pitch=puppy_limit(puppy_pitch+8000,-85000,85000);return 1;}
    if key==GUI_KEY_DOWN_ARROW {puppy_pitch=puppy_limit(puppy_pitch-8000,-85000,85000);return 1;}
    if key==GUI_KEY_ESCAPE {
        if puppy_draft && puppy_tool==-7 {puppy_tool=0;return puppy_title(cad_project_open(puppy_history+24),"Edge projection cancelled");}
        if puppy_revolve_axis {puppy_revolve_axis=0;puppy_anchor=0;return puppy_title(cad_project_open(puppy_history+24),"Revolve cancelled");}
        if puppy_draft && puppy_line_edit_active() {return puppy_line_edit_cancel(cad_project_open(puppy_history+24));}
        if puppy_picking_face {puppy_picking_face=0;return puppy_title(cad_project_open(puppy_history+24),0);}
        if puppy_anchor {puppy_anchor=0;return puppy_title(cad_project_open(puppy_history+24),puppy_draft_hint());}
        if puppy_saved_history==puppy_history || puppy_exit_requested {return 0;}
        puppy_exit_requested=1;
        return puppy_title(cad_project_open(puppy_history+24),"Unsaved edits. Save or close again to discard.");
    }
    puppy_exit_requested=0;
    if key==GUI_KEY_LEFT {puppy_yaw=puppy_yaw-8000;return 1;}
    if key==GUI_KEY_RIGHT {puppy_yaw=puppy_yaw+8000;return 1;}
    let project=cad_project_open(puppy_history+24);
    if key==61 && !(mods&0x3c3) && !puppy_sketch && cad_project_part(project) {
        let feature=puppy_feature(project);
        if m_eq(j_string(feature,"type"),"extrude") {return puppy_extrusion_begin(project,feature);}
    }
    if !cad_project_is_studio(project) && (mods&3) && !(mods&0x3c0) {
        if key==17 {return puppy_add_item(project,"part");}
        if key==4 {return puppy_add_item(project,"assembly");}
    }
    if key==22 && mods&3 && !(mods&0x3c0) && cad_project_part(project) && !puppy_draft {
        return puppy_start_plane_chooser(project);
    }
    if key==69 && !(mods&0x3c3) {return puppy_workspace_switch(project);}
    if key==9 && !(mods&0x3c0) && cad_project_part(project) && !puppy_draft {
        puppy_sketch=0;puppy_picking_face=!puppy_picking_face;
        if puppy_picking_face {return puppy_title(project,"Click a face for the new sketch");}
        return puppy_title(project,0);
    }
    if key==63 && !cad_project_is_studio(project) {return puppy_next_item(project);}
    if key==25 && !(mods&0x3c0) && puppy_pending_face_index(project)>=0 {
        if puppy_draft {puppy_draft=0;puppy_sketch=0;puppy_anchor=0;}
        else {puppy_pending_face(project);puppy_sketch=1;puppy_draft=1;}
        puppy_picking_face=0;return puppy_title(project,0);
    }
    if !puppy_draft && key==25 && !(mods&0x3c0) && cad_project_part(project) {
        if !puppy_sketch && !j_get(puppy_feature(project),"sketchId") {return puppy_title(project,"Select a sketch-based feature");}
        puppy_sketch=!puppy_sketch;return puppy_title(project,0);}
    if (mods&192) && (key==8 || (key==19 && cad_project_is_studio(project))) {
        let extension=".stl";if mods&768 {extension=".3mf";}else if mods&3 {extension=".step";}
        if key==19 {extension=".pdf";}
        return puppy_export_shortcut(project,extension);
    }
    if (mods&192) && key==22 {return puppy_save(project);}
    if (mods&192) && (key==29 || key==28) {
        let next=load64(puppy_history);
        if key==28 || (mods&3) {next=load64(puppy_history+8);}
        if next {
            let part=cad_project_part(project);let owner=j_string(part,"id");let sketch="";
            let keep_part=part && !puppy_sketch;
            if puppy_draft && puppy_explicit_sketch && !puppy_line_report {sketch=j_string(puppy_draft_sketch(project),"id");}
            project=cad_project_open(next+24);
            let next_part=cad_project_part(project);
            keep_part=keep_part && m_eq(owner,j_string(next_part,"id"))
                && !m_eq(j_encode(j_get(puppy_part_tree(part),"sketchVisibility")),
                         j_encode(j_get(puppy_part_tree(next_part),"sketchVisibility")));
            if puppy_refresh(project) {
                puppy_history=next;puppy_anchor=0;
                if keep_part {puppy_workspace_show_part(project);}
                if m_len(sketch) && m_eq(owner,j_string(cad_project_part(project),"id")) {
                    let index=puppy_workspace_sketch_index(project,sketch);
                    if index>=0 {puppy_draft_index=index;puppy_draft=1;puppy_sketch=1;puppy_empty=0;puppy_explicit_sketch=1;}
                    else {puppy_workspace_show_part(project);}
                }
            }else {project=cad_project_open(puppy_history+24);}
        }
        if puppy_patterns {puppy_sketch=1;puppy_draft=1;puppy_pattern_selection(0);return puppy_title(project,puppy_pattern_hint(project));}
        if puppy_line_report {
            puppy_symmetry_end();puppy_distance_cancel();let report=cad_project_line_sketch(project,j_string(puppy_draft_sketch(project),"id"));
            if !report {return puppy_line_close(project);}puppy_line_cache(report);return puppy_title(project,puppy_line_label());
        }
        if puppy_history_rows {return puppy_history_select(project,j_parse(puppy_history_rows+24));}
        return puppy_title(project,0);
    }
    if !puppy_draft && !(mods&0x3c0) && (key==59 || key==46 || key==45 || key==87 || key==86) {
        let field=puppy_active_field(project);if !field {return 1;}
        if key==59 {return puppy_number_open(project,field);}
        let change=1000000;if key==45 || key==86 {change=-change;}
        if !puppy_write_field(project,field,load64(field+8)+change) {return puppy_title(project,cg_error);}
        return puppy_title(cad_project_open(puppy_history+24),"Edited");
    }
    if cad_project_is_studio(project) {
        let updated=0;
        if key==43 {puppy_selected=puppy_selected+1;return puppy_title(project,0);}
        if key==19 && puppy_pick_id && j_get(project,"assembly") {
            updated=cad_studio_toggle_fixed(project,puppy_pick_id+24);
        }else {return 1;}
        if !updated || !puppy_upload(updated,cad_project_mesh(updated)) {return puppy_title(project,cg_error);}
        puppy_push(updated);return puppy_title(updated,"Edited");
    }
    if puppy_draft {if !(mods&0x3c0) {return puppy_draft_key(project,key,mods);}return 1;}
    let item=cad_project_item(project);
    if m_eq(j_string(item,"type"),"assembly") {
        if key==76 && !(mods&0x3c3) {return puppy_delete_selection(project);}
        if key==13 && !(mods&0x3c0) {return puppy_mate_begin(project,1);}
        if key==10 && !(mods&0x3c0) {return puppy_mate_ground(project);}
        if key==19 && !(mods&0x3c0) {
            puppy_part_choice=puppy_part_choice+1;let part=puppy_insert_part_choice(project);
            return puppy_title(project,m_cat("Insert part: ",j_string(part,"name")));
        }
        if key==12 && !(mods&0x3c0) {
            let part=puppy_insert_part_choice(project);
            let updated=cad_project_insert_instance(project,j_string(part,"id"));
            if !updated || !puppy_refresh(updated) {return puppy_title(project,cg_error);}
            puppy_push(updated);return puppy_title(updated,m_cat("Inserted ",j_string(part,"name")));
        }
        if key==43 {puppy_mate_selection(0);puppy_selected=puppy_selected+1;return puppy_title(project,0);}
        if key==25 && !(mods&0x3c0) {
            let updated=cad_project_solve_assembly(project);
            if !updated {return puppy_title(project,cg_error);}
            if !puppy_refresh(updated) {return puppy_title(project,"Cannot display assembly");}
            if !m_eq(j_encode(updated),j_encode(project)) {puppy_push(updated);}
            return puppy_title(updated,"Solved placements");
        }
        return 1;
    }
    if key==43 {
        if puppy_rectangle(project,puppy_feature(project)) {puppy_property=(puppy_property+1)%3;}
        else {puppy_property=0;}
        if !puppy_property {puppy_selected=puppy_selected+1;}
        if puppy_sketch && !j_get(puppy_feature(project),"sketchId") {puppy_sketch=0;}
        return puppy_title(project,0);
    }
    return 1;
}
fn puppy_pick_sketch(x,y) {
    let project=cad_project_open(puppy_history+24);
    let face=puppy_face_ray(project,x,y);
    if !face {return puppy_title(project,cg_error);}
    let updated=cad_project_add_face_sketch(project,face);
    if !updated || !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_tool=0;puppy_push(updated);return puppy_title(updated,puppy_draft_hint());
}
fn puppy_refresh(project) {
    puppy_part_sketch_dirty=1;
    puppy_revolve_axis=0;puppy_explicit_sketch=0;
    puppy_forget_click();puppy_clear_pick();puppy_search_focus(0);
    if puppy_isolated && !m_eq(puppy_isolated_owner+24,j_string(cad_project_item(project),"id")) {puppy_end_isolation();}
    puppy_picking_face=0;
    let item=cad_project_item(project);let data=j_get(item,"data");
    let instances=j_get(data,"instances");
    if !cad_project_part(project) {puppy_history_diagnostic(0);if puppy_history_rows {puppy_history_close();}}
    if m_eq(j_string(item,"type"),"assembly") && j_kind(instances)==3 && !pc_first(instances)
        && !cg_j_count(j_get(data,"mates")) && !cg_j_count(j_get(data,"actuators")) {
        puppy_end_isolation();puppy_empty=1;puppy_sketch=0;puppy_draft=0;return 1;
    }
    let mesh=0;
    if cad_project_part(project) {
        let preview=cad_project_feature_history(project);
        if j_get(preview,"error") {return 0;}
        let rows=j_get(preview,"features");mesh=j_get(preview,"mesh");puppy_history_diagnostic(rows);
        if puppy_history_rows {
            puppy_drop(puppy_history_rows);puppy_history_rows=puppy_snapshot(j_encode(rows));
        }
        if !mesh && pc_first(rows) {
            puppy_end_isolation();puppy_empty=1;puppy_sketch=0;puppy_draft=0;return 1;
        }
    }
    if !mesh {mesh=cad_project_mesh(project);}
    let error=j_get(mesh,"error");
    if !error {
        if !puppy_upload(project,mesh) {return 0;}puppy_empty=0;
        if puppy_pending_face(project) {puppy_sketch=1;puppy_draft=1;return 1;}
        if puppy_draft || !cad_project_part(project) {puppy_sketch=0;puppy_property=0;}puppy_draft=0;return 1;
    }
    if !m_eq(j_value(error),"Part has no evaluated solid") || puppy_feature(project) {return 0;}
    cg_begin();let sketch=puppy_draft_sketch(project);
    if !sketch {return cg_fail("Part has no sketch");}
    let paths=cad_sketch_paths(pc_nodes(cad_project_part(project)),j_string(sketch,"id"));
    if !paths {return 0;}puppy_end_isolation();puppy_empty=0;puppy_sketch=1;puppy_draft=1;return 1;
}
fn puppy_editor(path,source,project,report) {
    puppy_path=path;puppy_revision=j_integer(project,"revision");
    puppy_orthographic=cad_project_is_studio(project);
    if !puppy_view_open() {return puppy_error(gui_error);}
    puppy_loft_view=m_take(56);puppy_step_view=m_take(72);puppy_curve_coords=m_take(4096*16);let mark=m_mark();
    let normalized=0;
    if source && !j_get(report,"readOnly") {
        let rebuilt=cad_project_rebuild_projections(project);
        if rebuilt && rebuilt!=project && !m_eq(j_encode(project),j_encode(rebuilt)) {project=rebuilt;normalized=1;}
    }
    if !j_get(report,"readOnly") && !puppy_refresh(project) {
        if !source || !cg_error {
            let why=cg_error;if !why {why=gui_error;}if !why {why="Cannot display drawing";}
            let status=puppy_error(why);puppy_view_close();return status;
        }
        let message=puppy_snapshot(cg_error);m_reset(mark);cg_j_text(report,"readOnly",m_slice(message+24,load64(message+16)));
        puppy_drop(message);mark=m_mark();
    }
    if j_get(report,"readOnly") {
        if !puppy_reader_begin(report,source) {puppy_view_close();return puppy_error(gui_error);}
    }else if source && puppy_draft {puppy_fit_sketch(project,1);}
    if source {puppy_disk=puppy_snapshot(source);}
    puppy_projection_save_needed=normalized && !puppy_reader_report;
    if !source || puppy_projection_save_needed {source=j_encode(project);}
    puppy_history=puppy_snapshot(source);puppy_root=puppy_history;puppy_saved_history=puppy_history;
    if !puppy_reader_report {
        if puppy_step_input {puppy_step_begin(project);}
        else if cad_project_is_studio(project) {puppy_title(project,"PCAD design opened");}
        else if puppy_draft {puppy_title(project,puppy_draft_hint());}
        else if !cad_project_part(project) {puppy_title(project,"Assembly opened");}
        else if puppy_history_problem {puppy_title(project,puppy_history_problem+24);}
        else {puppy_title(project,"Tab selects a dimension");}
    }
    m_reset(mark);
    let event=gui_event();let running=1;let dirty=1;
    while running {
        while running && gui_poll(event) {
            let kind=gui_event_kind(event);let scratch=m_mark();
            if kind==GUI_KEY_DOWN || kind==GUI_KEY_UP {puppy_pattern_mods=gui_event_modifiers(event);}
            if kind==GUI_FOCUS && !gui_event_code(event) {puppy_pattern_mods=0;}
            if kind==GUI_MOUSE_MOVE || kind==GUI_MOUSE_DOWN {
                let x=gui_event_x(event);let y=gui_event_y(event);
                if puppy_breadcrumb_target(x,y)!=puppy_breadcrumb_target(puppy_pointer_x,puppy_pointer_y) {
                    puppy_toolbar_dirty=1;dirty=1;
                }
                puppy_pointer_x=x;puppy_pointer_y=y;
            }
            if puppy_reader_report {
                let result=puppy_reader_event(event);running=result!=0;
                if result==2 {dirty=1;}
            }
            else if puppy_construction_ids && kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 {
                let x=gui_event_x(event);let y=gui_event_y(event);let current=cad_project_open(puppy_history+24);
                if y>=76 {puppy_construction_pick(current,x,y);}
                else if y>=42 && y<68 {puppy_action_bar(puppy_construction_actions(),0,gui_width(load64(puppy_device)),x);}
                dirty=1;
            }
            else if puppy_number_event(event) {dirty=1;}
            else if puppy_feature_preview_event(event) {dirty=1;}
            else if puppy_mate_event(event) {dirty=1;}
            else if puppy_step_event(event) {dirty=1;}
            else if puppy_pattern_event(event) {dirty=1;}
            else if puppy_plane_chooser && puppy_plane_event(event) {if kind==GUI_MOUSE_DOWN {dirty=1;}}
            else if puppy_design_available() && puppy_objects_event(event) {dirty=1;}
            else if puppy_sheet_scene && (kind==GUI_QUIT || kind==GUI_CLOSE) {
                puppy_sheet_close();running=puppy_command(GUI_KEY_ESCAPE,0);dirty=1;
            }
            else if puppy_sheet_scene && puppy_sheet_event(event) {if kind==GUI_MOUSE_DOWN {dirty=1;}}
            else if puppy_join_event(event) {dirty=1;}
            else if puppy_line_event(event) {dirty=1;}
            else if puppy_context_event(event) {dirty=1;}
            else if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 && !puppy_button && puppy_cube_click(puppy_pointer_x,puppy_pointer_y) {
                puppy_search_focus(0);puppy_forget_click();dirty=1;
            }else if !puppy_design_available() && !puppy_history_rows && puppy_objects_event(event) {dirty=1;}
            else if kind==GUI_QUIT || kind==GUI_CLOSE {running=puppy_command(GUI_KEY_ESCAPE,0);dirty=1;}
            else if kind==GUI_KEY_DOWN {running=puppy_command(gui_event_code(event),gui_event_modifiers(event));dirty=1;}
            else if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 {
                let x=gui_event_x(event);let y=gui_event_y(event);
                let width=gui_width(load64(puppy_device));
                if y<76 {puppy_forget_click();}
                if puppy_history_rows {
                    let current=cad_project_open(puppy_history+24);
                    if y>=42 && y<68 {puppy_compact_bar(0,width,current,x);}
                    else if x<puppy_objects_width() && y>=76 {puppy_history_click(current,y);}
                    dirty=1;
                }
                else if puppy_edge_kind {
                    let current=cad_project_open(puppy_history+24);
                    if y>=42 && y<68 {
                        if width<800 {puppy_compact_bar(0,width,current,x);}
                        else {puppy_action_bar(puppy_edge_actions(),0,width,x);}
                    }else if y>=76 {puppy_edge_pick(current,x,y);}
                    dirty=1;
                }
                else if !puppy_sketch && !puppy_empty && width>=800 && y>=6 && y<34 && x>=width-420 && x<width-335 && cad_project_part(cad_project_open(puppy_history+24)) {
                    puppy_command(11,0);dirty=1;
                }
                else if puppy_loft_open {
                    if y>=42 && y<68 {
                        if width<800 {puppy_compact_bar(0,width,project,x);}
                        else {puppy_action_bar(puppy_loft_actions(),0,width,x);}
                    }
                    dirty=1;
                }
                else if puppy_breadcrumb_click(x,y) {dirty=1;}
                else if width>=800 && y>=6 && y<34 && x>=width-505 && x<width-429 && cad_project_part(cad_project_open(puppy_history+24)) {
                    puppy_command(38,0);dirty=1;
                }
                else if puppy_isolated && x>=12 && x<98 && y>=6 && y<37 {puppy_command(68,0);dirty=1;}
                else if !puppy_sketch && !puppy_empty && y>=6 && y<37 && puppy_objects_button(x) {puppy_command(23,0);dirty=1;}
                else if width>=800 && y>=6 && y<34 && puppy_value_button_hit(x) {
                    puppy_command(59,0);dirty=1;
                }
                else if puppy_draft && width>=800 && y>=6 && y<34 && x>=12 && x<88 {puppy_command(37,0);dirty=1;}
                else if puppy_draft && width>=800 && y>=6 && y<37 && x>=width-420 && x<width-335 {
                    puppy_command(10,0);dirty=1;
                }
                else if cad_project_is_studio(project) && width>=800 && y>=6 && y<37 && x>=width-140 && x<width-12 {
                    puppy_command(18,0);dirty=1;
                }else if cad_project_is_studio(project) && width>=800 && y>=6 && y<37 && x>=width-260 && x<width-148 {
                    puppy_command(12,192);dirty=1;
                }else if !cad_project_is_studio(project) && width>=800 && y>=6 && y<37 && x>=width-109 && x<width-12 {
                    if puppy_draft {puppy_command(69,0);}else {puppy_command(63,0);}
                    dirty=1;
                }else if !cad_project_is_studio(project) && width>=800 && y>=6 && y<37 && x>=width-235 && x<width-117 {
                    puppy_command(4,3);dirty=1;
                }else if !cad_project_is_studio(project) && width>=800 && y>=6 && y<37 && x>=width-327 && x<width-243 {
                    puppy_command(17,3);dirty=1;
                }else if width<800 && y>=42 && y<68 {
                    puppy_compact_bar(0,width,cad_project_open(puppy_history+24),x);dirty=1;
                }else if y>=42 && y<68 {
                    let current=cad_project_open(puppy_history+24);
                    if (puppy_draft || puppy_camera_available()) && x>=width-60 && x<width-12 {puppy_command(74,0);}
                    else if !puppy_draft && !puppy_empty && x>=width-132 && x<width-68 {puppy_command(8,195);}
                    else if !puppy_draft && cad_project_part(current) && x>=568 && x<648 {puppy_command(64,0);}
                    else if !puppy_draft && width>=900 && x>=656 && x<746 && puppy_feature(current) && puppy_line_button(current) {puppy_command(65,0);}
                    else if x>=12 && x<64 {puppy_command(29,192);}
                    else if x>=70 && x<122 {puppy_command(28,192);}
                    else if x>=128 && x<180 {puppy_command(22,192);}
                    else if cad_project_is_studio(current) {
                        if x>=194 && x<232 {puppy_command(45,0);}
                        else if x>=238 && x<276 {puppy_command(46,0);}
                        else if x>=288 && x<366 {puppy_command(43,0);}
                        else if x>=376 && x<444 {puppy_command(8,192);}
                        else if x>=452 && x<549 {puppy_command(19,0);}
                    }
                    else if puppy_draft {
                        let pattern_x=628;
                        if puppy_line_button(current) {pattern_x=726;}if puppy_draft_on_face(current) {pattern_x=721;}if puppy_pending_face_index(current)>=0 {pattern_x=799;}
                        if width>=920 && !puppy_revolve_axis && !puppy_curves_page && x>=pattern_x && x<pattern_x+80 {puppy_command(62,0);}
                        else if !puppy_revolve_axis && !puppy_curves_page && x>=628 && x<718 && puppy_line_button(current) {puppy_command(65,0);}
                        else if puppy_revolve_axis {
                            if x>=194 && x<270 {puppy_command(27,0);}
                            else if x>=278 && x<354 {puppy_command(28,0);}
                            else if x>=362 && x<438 {puppy_command(37,0);}
                        }else if puppy_curves_page {puppy_curve_bar(current,0,width,x);}
                        else if x>=194 && x<280 {puppy_command(21,0);}
                        else if x>=288 && x<355 {puppy_command(6,0);}
                        else if x>=364 && x<445 {puppy_command(27,0);}
                        else if x>=452 && x<552 {puppy_command(19,0);}
                        else if x>=560 && x<620 {puppy_command(15,0);}
                        else if x>=628 && x<713 {puppy_command(18,0);}
                        else if x>=721 && x<791 && puppy_pending_face_index(current)>=0 {puppy_command(25,0);}
                    }
                    else if x>=194 && x<232 {puppy_command(45,0);}
                    else if x>=238 && x<276 {puppy_command(46,0);}
                    else if x>=288 && x<372 {
                        if cad_project_part(current) && puppy_draft_sketch(current) {puppy_command(69,0);}
                        else if !puppy_empty && x<366 {puppy_command(25,0);}
                    }
                    else if !puppy_empty && x>=376 && x<444 {puppy_command(8,192);}
                    else if x>=452 && x<521 && !cad_project_part(current) {puppy_command(12,0);}
                    else if !puppy_empty && x>=width-211 && x<width-140 && m_eq(j_string(cad_project_item(current),"type"),"assembly") {puppy_command(13,0);}
                    else if x>=529 && x<699 && !cad_project_part(current) {
                        let data=j_get(cad_project_item(current),"data");
                        if puppy_servo(data) && x>=625 {puppy_command(43,0);}else {puppy_command(19,0);}
                    }
                    else if x>=452 && x<560 && cad_project_part(current) {puppy_command(22,3);}
                    else if x>=452 {puppy_command(43,0);}
                    dirty=1;
                }
                else if puppy_picking_face && y>=76 {puppy_pick_sketch(x,y);dirty=1;}
                else if puppy_draft && y>=76 {puppy_draft_click(x,y);dirty=1;}
                else if !puppy_sketch && !puppy_empty && y>=76 {puppy_view_down(1,x,y);dirty=1;}
            }
            else if kind==GUI_MOUSE_DOWN && puppy_camera_available() && gui_event_y(event)>=76 {
                let button=gui_event_button(event);
                if button==2 || button==3 {puppy_view_down(button,gui_event_x(event),gui_event_y(event));}
            }
            else if kind==GUI_MOUSE_MOVE && puppy_button {puppy_view_move(gui_event_x(event),gui_event_y(event));dirty=1;}
            else if kind==GUI_MOUSE_UP && puppy_button==gui_event_button(event) {
                puppy_view_move(gui_event_x(event),gui_event_y(event));puppy_view_end(1);dirty=1;
            }
            else if kind==GUI_FOCUS && !gui_event_code(event) && puppy_button {puppy_view_end(0);dirty=1;}
            else if kind==GUI_MOUSE_MOVE && puppy_anchor {
                let p=puppy_draft_point(gui_event_x(event),gui_event_y(event));
                puppy_mx=load64(p);puppy_my=load64(p+8);dirty=1;
            }
            else if kind==GUI_WHEEL && puppy_draft {
                let step=puppy_limit(puppy_draft_scale/4,1,1000);
                puppy_draft_scale=puppy_limit(puppy_draft_scale+gui_event_dy(event)*step,1,32000);dirty=1;
            }else if kind==GUI_WHEEL && !puppy_button {
                puppy_forget_click();
                puppy_zoom=puppy_zoom-gui_event_dy(event)*15;
                if puppy_zoom<120 {puppy_zoom=120;}if puppy_zoom>800 {puppy_zoom=800;}dirty=1;
            }else if kind==GUI_RESIZE || kind==GUI_EXPOSE {dirty=1;}
            m_reset(scratch);
        }
        if running && dirty {
            let scratch=m_mark();
            if !puppy_draw() {puppy_error(gui_error);running=0;}
            else if puppy_pending_title {
                // Publish the edit status once its updated frame has been presented.
                gui_title(load64(puppy_device),puppy_pending_title+24);
                puppy_drop(puppy_pending_title);puppy_pending_title=0;
            }
            m_reset(scratch);dirty=0;
        }
        if running {gui_wait(100);}
    }
    if puppy_reader_offsets {syscall(11,puppy_reader_offsets,(puppy_reader_count+1)*8,0,0,0,0);}
    puppy_distance_cancel();puppy_line_cancel_drag();if puppy_line_report {puppy_drop(puppy_line_report);}
    if puppy_number_text {puppy_number_close();}
    puppy_extrusion_close();puppy_revolve_close();
    puppy_workspace_clear_parent();
    puppy_sheet_close();puppy_step_clear();puppy_join_end_drag();puppy_join_select(0);
    if puppy_pending_title {puppy_drop(puppy_pending_title);}
    puppy_forget_click();puppy_end_isolation();puppy_clear_pick();puppy_trim(puppy_root);if puppy_disk {puppy_drop(puppy_disk);}if puppy_status {puppy_drop(puppy_status);}puppy_view_close();return 0;
}
