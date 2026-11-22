import "cad/project-arc-edit.flex";
import "cad/project-ellipse-edit.flex";
import "cad/project-spline-edit.flex";

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

fn puppy_line_edit_distance(row,point,tolerance,tool) {
    let kind=j_string(row,"type");let picked=0;
    if tool==-9 && !m_eq(kind,"sketchLine") && !pc_finite_arc(row) {return tolerance+1;}
    if pc_ellipse_entity(row) {
        if tool==-9 && cg_abs(pc_serial_number(j_get(pc_curve_geometry(row),"sweep"),12,1000000000000,7))>=6283185307180 {
            return tolerance+1;
        }
        picked=cad_sketch_elliptic_pick(row,point,tolerance);
    }else if m_eq(kind,"sketchSpline") {
        picked=cad_sketch_spline_pick(row,point,tolerance);
        let spline=j_get(row,"spline");
        if picked && tool==-6 && !j_value(j_get(spline,"closed")) {
            let spans=cg_j_count(j_get(spline,"points"))-1;
            if m_eq(j_string(spline,"mode"),"control") {spans=spans/3;}
            let progress=load64(picked);
            if progress<=1000000 || progress>=spans*1000000000000-1000000 {return tolerance+1;}
        }
    }else if tool!=-6 {
        if m_eq(kind,"sketchLine") {return puppy_segment_distance(point,pc_uv(j_get(row,"p0")),pc_uv(j_get(row,"p1")));}
        picked=cad_sketch_circular_pick(row,point,tolerance);
    }
    if picked {return load64(picked+8);}
    return tolerance+1;
}

fn puppy_line_edit_target(nodes,sketch,point,tolerance,tool) {
    let row=pc_first(nodes);let selected=0;let best=tolerance+1;
    while row && !cg_error {
        if m_eq(j_string(row,"sketchId"),sketch) {
            let distance=puppy_line_edit_distance(row,point,tolerance,tool);
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
    let target=puppy_line_edit_target(nodes,sketch,point,tolerance,puppy_tool);
    if cg_error {return puppy_title(project,cg_error);}
    if !target && puppy_tool==-9 {
        if puppy_line_edit_target(nodes,sketch,point,tolerance,-8) {return puppy_title(project,"Choose a line or an open arc to extend");}
        if cg_error {return puppy_title(project,cg_error);}
    }
    if !target {return puppy_title(project,puppy_line_edit_hint());}
    let id=j_string(target,"id");let line=m_eq(j_string(target,"type"),"sketchLine");let extend=puppy_tool==-9;
    let updated=0;let message="Arc trimmed";
    if line {
        if extend {updated=cad_project_extend_line(project,sketch,id,point,tolerance);message="Line extended";}
        else {updated=cad_project_trim_line(project,sketch,id,point,tolerance);message="Line trimmed";}
    }else if pc_ellipse_entity(target) {
        if extend {updated=cad_project_extend_elliptic_arc(project,sketch,id,point,tolerance);message="Elliptic arc extended";}
        else {
            updated=cad_project_trim_elliptic_curve(project,sketch,id,point,tolerance);message="Ellipse trimmed";
            if pc_finite_arc(target) {message="Elliptic arc trimmed";}
        }
    }else if m_eq(j_string(target,"type"),"sketchSpline") {
        updated=cad_project_trim_spline(project,sketch,id,point,tolerance);message="Spline trimmed";
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
