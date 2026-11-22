import "cad/project-arc-edit.flex";

fn puppy_line_edit_active() {return puppy_tool==-8 || puppy_tool==-9;}

fn puppy_line_edit_hint() {
    if puppy_tool==-9 {return "Click near the end to extend";}
    return "Click the curve segment to trim";
}

fn puppy_line_edit_begin(project,extend) {
    puppy_tool=-8;if extend {puppy_tool=-9;}
    puppy_curves_page=4;puppy_anchor=0;
    return puppy_title(project,puppy_line_edit_hint());
}

fn puppy_line_edit_cancel(project) {
    puppy_tool=0;puppy_curves_page=0;
    return puppy_title(project,"Curve edit cancelled");
}

fn puppy_line_edit_target(nodes,sketch,point,tolerance) {
    let row=pc_first(nodes);let selected=0;let best=tolerance+1;
    while row {
        if m_eq(j_string(row,"sketchId"),sketch) {
            let distance=best;
            if m_eq(j_string(row,"type"),"sketchLine") {
                distance=puppy_segment_distance(point,pc_uv(j_get(row,"p0")),pc_uv(j_get(row,"p1")));
            }else {
                let picked=cad_sketch_circular_pick(row,point,tolerance);
                if picked {distance=load64(picked+8);}
            }
            if distance<best {best=distance;selected=row;}
        }
        row=load64(row+24);
    }
    return selected;
}

fn puppy_line_edit_click(project,point) {
    cg_begin();let nodes=pc_nodes(cad_project_part(project));
    let sketch=j_string(puppy_draft_sketch(project),"id");
    let tolerance=puppy_limit(10000000000/puppy_draft_scale,1,500000000);
    let target=puppy_line_edit_target(nodes,sketch,point,tolerance);
    if !target {return puppy_title(project,puppy_line_edit_hint());}
    let id=j_string(target,"id");let line=m_eq(j_string(target,"type"),"sketchLine");let extend=puppy_tool==-9;
    let updated=0;let message="Arc trimmed";
    if line {
        if extend {updated=cad_project_extend_line(project,sketch,id,point,tolerance);message="Line extended";}
        else {updated=cad_project_trim_line(project,sketch,id,point,tolerance);message="Line trimmed";}
    }else if extend {
        updated=cad_project_extend_circular_arc(project,sketch,id,point,tolerance);message="Arc extended";
    }else {
        updated=cad_project_trim_circular_curve(project,sketch,id,point,tolerance);
        if pc_sketch_circle(target) {message="Circle trimmed";}
    }
    if !updated {return puppy_title(project,cg_error);}
    puppy_profile_index=1;
    puppy_push(updated);
    return puppy_title(updated,message);
}
