import "cad/project-line-edit.flex";

fn puppy_line_edit_active() {return puppy_tool==-8 || puppy_tool==-9;}

fn puppy_line_edit_hint() {
    if puppy_tool==-9 {return "Click a line near the end to extend";}
    return "Click the line segment to trim";
}

fn puppy_line_edit_begin(project,extend) {
    puppy_tool=-8;if extend {puppy_tool=-9;}
    puppy_curves_page=4;puppy_anchor=0;
    return puppy_title(project,puppy_line_edit_hint());
}

fn puppy_line_edit_cancel(project) {
    puppy_tool=0;puppy_curves_page=0;
    return puppy_title(project,"Line edit cancelled");
}

fn puppy_line_edit_click(project,point) {
    let nodes=pc_nodes(cad_project_part(project));
    let sketch=j_string(puppy_draft_sketch(project),"id");
    let line=puppy_mirror_hit(nodes,sketch,point,1);
    if !line {return puppy_title(project,puppy_line_edit_hint());}
    let id=j_string(line,"id");
    let tolerance=puppy_limit(10000000000/puppy_draft_scale,1,500000000);
    let updated=0;let message="Line trimmed";
    if puppy_tool==-9 {
        updated=cad_project_extend_line(project,sketch,id,point,tolerance);message="Line extended";
    }else {updated=cad_project_trim_line(project,sketch,id,point,tolerance);}
    if !updated {return puppy_title(project,cg_error);}
    puppy_profile_index=1;
    puppy_push(updated);
    return puppy_title(updated,message);
}
