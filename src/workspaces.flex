global puppy_explicit_sketch=0;

fn puppy_workspace_heading(project) {
    let item=cad_project_item(project);
    let name=puppy_document_name(project);
    if puppy_draft {
        return m_cat("Sketch / ",puppy_design_name(puppy_draft_sketch(project),"Sketch"));
    }
    if puppy_sketch {return m_cat("Sketch preview / ",name);}
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
