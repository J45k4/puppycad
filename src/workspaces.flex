global puppy_explicit_sketch=0;
global puppy_plane_chooser=0;
// Parts are top-level records. Remember the assembly used to reach this part.
global puppy_workspace_parent_id=0;
global puppy_workspace_part_id=0;

fn puppy_workspace_clear_parent() {
    if puppy_workspace_parent_id {puppy_drop(puppy_workspace_parent_id);puppy_workspace_parent_id=0;}
    if puppy_workspace_part_id {puppy_drop(puppy_workspace_part_id);puppy_workspace_part_id=0;}
}

fn puppy_workspace_track_parent(previous,part) {
    puppy_workspace_clear_parent();
    if !part || !m_eq(j_string(previous,"type"),"assembly") {return 0;}
    let id=j_string(part,"id");let record=pc_first(j_get(j_get(previous,"data"),"instances"));
    while record {
        if m_eq(j_string(record,"partId"),id) {
            puppy_workspace_parent_id=puppy_snapshot(j_string(previous,"id"));
            puppy_workspace_part_id=puppy_snapshot(id);
            return 1;
        }
        record=load64(record+24);
    }
    return 0;
}

fn puppy_workspace_parent(project) {
    if !puppy_workspace_parent_id || !m_eq(j_string(cad_project_part(project),"id"),puppy_workspace_part_id+24) {return 0;}
    let item=pc_find(j_get(project,"items"),puppy_workspace_parent_id+24);
    if m_eq(j_string(item,"type"),"assembly") {return item;}
    return 0;
}

fn puppy_workspace_return_to_assembly(project) {
    let assembly=puppy_workspace_parent(project);
    if !assembly {return 0;}
    if !puppy_open_item(project,puppy_item_index(project,assembly)) {
        return puppy_title(cad_project_open(puppy_history+24),"Cannot open this assembly");
    }
    puppy_design_select_item(assembly);
    return puppy_title(project,m_cat("Opened ",puppy_design_item_name(assembly,"assembly")));
}

fn puppy_workspace_heading(project) {
    let item=cad_project_item(project);
    let name=puppy_document_name(project);
    if puppy_draft {
        return m_cat("Sketch / ",puppy_design_name(puppy_draft_sketch(project),"Sketch"));
    }
    if puppy_sketch {return "Sketch preview";}
    if m_eq(j_string(item,"type"),"assembly") || j_get(project,"assembly") {
        return m_cat("Assembly / ",name);
    }
    return m_cat("Part / ",name);
}

fn puppy_workspace_sketch_index(project,id) {
    let node=pc_first(pc_nodes(cad_project_part(project)));let index=0;
    while node {
        if m_eq(j_string(node,"type"),"sketch") {
            if m_eq(j_string(node,"id"),id) {return index;}
            index=index+1;
        }
        node=load64(node+24);
    }
    return -1;
}

fn puppy_workspace_edit_sketch(project,id) {
    let index=puppy_workspace_sketch_index(project,id);
    if index<0 {return puppy_title(project,"Select a sketch in the Design tree");}
    puppy_end_isolation();puppy_clear_pick();puppy_search_focus(0);puppy_design_focus=0;
    puppy_picking_face=0;puppy_revolve_axis=0;puppy_anchor=0;
    puppy_tool=0;puppy_curves_page=0;puppy_draft_index=index;puppy_profile_index=1;puppy_face_operation=0;
    puppy_draft=1;puppy_sketch=1;puppy_empty=0;puppy_explicit_sketch=1;puppy_bar_page=0;
    puppy_fit_sketch(project,1);
    return puppy_title(project,m_cat("Editing ",puppy_design_name(puppy_draft_sketch(project),"Sketch")));
}

// Refresh may automatically show a blank or pending face sketch on opening a file.
// Entering Part explicitly leaves that sketch available from the Design tree.
fn puppy_workspace_show_part(project) {
    if puppy_draft && puppy_pending_face_index(project)<0 {puppy_empty=1;}
    puppy_draft=0;puppy_sketch=0;puppy_anchor=0;puppy_revolve_axis=0;puppy_bar_page=0;
    return 1;
}

fn puppy_workspace_switch(project) {
    if !cad_project_part(project) {return 1;}
    if puppy_draft {
        let index=puppy_draft_index;
        if !puppy_refresh(project) {return puppy_title(project,cg_error);}
        puppy_workspace_show_part(project);puppy_draft_index=index;
        return puppy_title(project,"Part workspace");
    }
    if puppy_sketch {
        puppy_sketch=0;return puppy_title(project,"Part workspace");
    }
    let sketch=puppy_draft_sketch(project);
    return puppy_workspace_edit_sketch(project,j_string(sketch,"id"));
}

fn puppy_workspace_return_to_part(project) {
    let result=puppy_workspace_switch(project);
    if !puppy_sketch {puppy_design_select_item(cad_project_part(project));}
    return result;
}

fn puppy_plane_actions() {
    let rows=cg_array();
    puppy_compact_button(rows,"Top (XY)",30,0);
    puppy_compact_button(rows,"Front (XZ)",31,0);
    puppy_compact_button(rows,"Right (YZ)",32,0);
    if !puppy_empty {puppy_compact_button(rows,"On face",9,0);}
    puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);
    return rows;
}
fn puppy_start_plane_chooser(project) {
    puppy_search_focus(0);puppy_design_focus=0;puppy_tree_open=0;
    puppy_plane_chooser=1;puppy_picking_face=0;
    return puppy_title(project,"Choose a plane for the new sketch");
}
fn puppy_plane_command(project,key,mods) {
    if key==GUI_KEY_ESCAPE {
        puppy_plane_chooser=0;return puppy_title(project,"New sketch cancelled");
    }
    if mods&0x3c3 {return 1;}
    if key==9 && !puppy_empty {
        puppy_plane_chooser=0;
        puppy_sketch=0;puppy_picking_face=1;
        return puppy_title(project,"Click a face for the new sketch");
    }
    let plane="";
    if key==30 {plane="XY";}
    if key==31 {plane="XZ";}
    if key==32 {plane="YZ";}
    if !m_len(plane) {return 1;}
    let updated=cad_project_add_plane_sketch(project,plane);
    if !updated {return puppy_title(project,cg_error);}
    let row=pc_first(pc_nodes(cad_project_part(updated)));
    while load64(row+24) {row=load64(row+24);}
    puppy_plane_chooser=0;puppy_push(updated);
    puppy_draft_cx=0;puppy_draft_cy=0;puppy_draft_scale=4000;
    return puppy_workspace_edit_sketch(updated,j_string(row,"id"));
}

fn puppy_plane_event(event) {
    let kind=gui_event_kind(event);
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 {
        let y=gui_event_y(event);
        if y>=42 && y<68 {
            puppy_action_bar(puppy_plane_actions(),0,gui_width(load64(puppy_device)),gui_event_x(event));
        }
    }
    return kind==GUI_MOUSE_DOWN || kind==GUI_MOUSE_UP || kind==GUI_MOUSE_MOVE || kind==GUI_WHEEL;
}
