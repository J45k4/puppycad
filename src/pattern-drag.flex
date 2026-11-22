global puppy_pattern_drag=0;
global puppy_pattern_preview=0;
global puppy_pattern_down_x=0;
global puppy_pattern_down_y=0;
global puppy_pattern_last_angle=0;
global puppy_pattern_drag_angle=0;
global puppy_pattern_mods=0;
global puppy_pattern_moved=0;
global PUPPY_COLUMN_STEP=1;
global PUPPY_ROW_STEP=2;
global PUPPY_RING_CENTER=3;
global PUPPY_RING_ANGLE=4;

// The first source defines the position of the pattern controls, as in the web editor.
fn puppy_pattern_origin(project,rel) {
    let nodes=pc_nodes(cad_project_part(project));let src=pc_find(nodes,j_value(pc_first(j_get(rel,"sources"))));
    let type=j_string(src,"type");
    if m_eq(type,"sketchLine") || m_eq(type,"sketchCornerRectangle") {return pc_uv(j_get(src,"p0"));}
    if m_eq(type,"sketchSpline") {return pc_uv(pc_first(j_get(j_get(src,"spline"),"points")));}
    let geom=pc_curve_geometry(src);let coord=j_get(geom,"center");if !coord {coord=j_get(geom,"from");}
    if !coord {return 0;}return pc_uv(coord);
}
fn puppy_pattern_angle_at(point,center) {
    return pc_angle_degrees(load64(point+8)-load64(center+8),load64(point)-load64(center));
}
// Each handle carries its position, gesture and guide polyline in sketch coordinates.
fn puppy_pattern_handles(project,rel) {
    let handles=cg_array();if !rel {return handles;}
    let origin=puppy_pattern_origin(project,rel);if !origin {return handles;}
    if pp_kind(rel)==1 {
        let count=cg_j_count(j_get(rel,"instances"))+1;let cols=pp_columns(rel,count);
        let axis=PUPPY_COLUMN_STEP;while axis<=PUPPY_ROW_STEP {
            let n=cols;let step=0;
            if axis==PUPPY_ROW_STEP {n=count/cols;step=pp_row_step(rel);}else {step=pc_uv(j_get(rel,"step"));}
            if n>1 {
                let pos=cg_v(load64(origin)+load64(step),load64(origin+8)+load64(step+8),0);
                let guide=cg_array();cg_push(guide,origin);cg_push(guide,pos);
                cg_push(handles,cg_v(pos,axis,guide));
            }axis=axis+1;
        }
    }else {
        let center=pc_uv(j_get(rel,"center"));cg_push(handles,cg_v(center,PUPPY_RING_CENTER,0));
        let radius=pc_line_span(origin,center)+18000000000/puppy_draft_scale;
        if radius<40000000000/puppy_draft_scale {radius=40000000000/puppy_draft_scale;}
        let start=puppy_pattern_angle_at(origin,center);let angle=pc_pose_number(j_get(rel,"angle"));
        let n=2+cg_abs(angle)/8000000;let path=cg_array();let i=0;let pos=0;
        while i<=n {
            let a=pc_radians(start+angle*i/n);
            pos=cg_v(load64(center)+po_scaled(radius,f64_cos(a)),load64(center+8)+po_scaled(radius,f64_sin(a)),0);
            cg_push(path,pos);i=i+1;
        }
        cg_push(handles,cg_v(pos,PUPPY_RING_ANGLE,path));
    }return handles;
}
fn puppy_pattern_guides(project,rel,color,w,h) {
    let handles=puppy_pattern_handles(project,rel);let dots=cg_array();let i=0;
    render_gl(puppy_device,"glUniform1i",color,4,0,0,0,0);
    while i<cg_count(handles) {
        let row=cg_at(handles,i);let guide=load64(row+16);
        if guide && !puppy_draft_path(guide,3,w,h) {return 0;}
        cg_push(dots,load64(row));i=i+1;
    }
    if !cg_count(dots) {return 1;}
    render_gl(puppy_device,"glUniform1i",color,5,0,0,0,0);
    render_gl(puppy_device,"glEnable",0x8642,0,0,0,0,0);
    let ok=puppy_draft_path(dots,0,w,h);render_gl(puppy_device,"glDisable",0x8642,0,0,0,0,0);return ok;
}
fn puppy_pattern_down(project,x,y) {
    let sk=j_string(puppy_draft_sketch(project),"id");let model=puppy_sketch_model(project,sk);
    if !model {return puppy_title(project,cg_error);}
    if puppy_pattern_center {return puppy_pattern_click(project,model,x,y);}
    let rel=puppy_pattern_rel(project);let handles=puppy_pattern_handles(model,rel);let at=puppy_draft_point(x,y);
    let i=0;let best=65;let handle=0;while i<cg_count(handles) {
        let row=cg_at(handles,i);let p=load64(row);
        let dx=(load64(p)-load64(at))*puppy_draft_scale/1000000000;let dy=(load64(p+8)-load64(at+8))*puppy_draft_scale/1000000000;
        let d=dx*dx+dy*dy;if d<best {best=d;handle=row;}i=i+1;
    }
    if !handle {return puppy_pattern_click(project,model,x,y);}
    puppy_pattern_moved=0;puppy_pattern_drag=load64(handle+8);puppy_pattern_down_x=x;puppy_pattern_down_y=y;
    if puppy_pattern_drag==PUPPY_RING_ANGLE {
        puppy_pattern_last_angle=puppy_pattern_angle_at(at,pc_uv(j_get(rel,"center")));puppy_pattern_drag_angle=pc_pose_number(j_get(rel,"angle"));
    }
    return 1;
}
fn puppy_pattern_clear_preview() {
    if puppy_pattern_preview {puppy_drop(puppy_pattern_preview);puppy_pattern_preview=0;}return 1;
}
fn puppy_pattern_cancel(project) {
    puppy_pattern_clear_preview();puppy_pattern_drag=0;return puppy_title(project,"Pattern drag cancelled");
}
fn puppy_pattern_drag_edit(project,x,y) {
    let rel=puppy_pattern_rel(project);let at=puppy_draft_point(x,y);let settings=j_object();
    let dx=(x-puppy_pattern_down_x)*1000000000/puppy_draft_scale;let dy=(puppy_pattern_down_y-y)*1000000000/puppy_draft_scale;
    if puppy_pattern_drag==PUPPY_RING_ANGLE {
        let center=pc_uv(j_get(rel,"center"));
        if cg_abs(load64(at)-load64(center))<4000000000/puppy_draft_scale && cg_abs(load64(at+8)-load64(center+8))<4000000000/puppy_draft_scale {return cg_fail("Move away from the rotation center");}
        let angle=puppy_pattern_angle_at(at,center);let delta=angle-puppy_pattern_last_angle;
        if delta>180000000 {delta=delta-360000000;}if delta< -180000000 {delta=delta+360000000;}
        puppy_pattern_last_angle=angle;puppy_pattern_drag_angle=puppy_limit(puppy_pattern_drag_angle+delta,-360000000,360000000);
        angle=puppy_pattern_drag_angle;if puppy_pattern_mods&3 {
            angle=f64_to_i64_round(f64_div(f64_from_i64(angle),f64_from_i64(15000000)))*15000000;
        }
        if cg_abs(angle)>359500000 {if angle<0 {angle=-360000000;}else {angle=360000000;}}
        if cg_abs(angle)<100000 {if angle<0 || !angle && pc_pose_number(j_get(rel,"angle"))<0 {angle=-100000;}else {angle=100000;}}
        cg_j_set(settings,"angle",cg_json_number(angle));
    }else {
        let key="center";let base=0;
        if puppy_pattern_drag==PUPPY_COLUMN_STEP {key="step";}
        if puppy_pattern_drag==PUPPY_ROW_STEP {key="rowStep";base=pp_row_step(rel);}else {base=pc_uv(j_get(rel,key));}
        if puppy_pattern_drag!=PUPPY_RING_CENTER && puppy_pattern_mods&3 {
            let vx=load64(base);let vy=load64(base+8);
            let scale=f64_div(f64_add(f64_mul(f64_from_i64(dx),f64_from_i64(vx)),f64_mul(f64_from_i64(dy),f64_from_i64(vy))),f64_from_i64(vx*vx+vy*vy));
            dx=po_scaled(vx,scale);dy=po_scaled(vy,scale);
        }
        let pt=pc_json_point(cg_v(load64(base)+dx,load64(base+8)+dy,0));
        if puppy_pattern_drag==PUPPY_RING_CENTER {
            let sk=j_string(puppy_draft_sketch(project),"id");
            let model=puppy_sketch_model(project,sk);if !model {return 0;}
            return puppy_sketch_result(project,sk,cad_project_move_pattern_center(model,sk,j_string(rel,"id"),pt));
        }
        j_add(settings,key,pt);
    }
    return puppy_pattern_change(project,settings);
}
fn puppy_pattern_motion(project,x,y) {
    puppy_pattern_clear_preview();
    if x!=puppy_pattern_down_x || y!=puppy_pattern_down_y {puppy_pattern_moved=1;}
    if !puppy_pattern_moved || x==puppy_pattern_down_x && y==puppy_pattern_down_y && puppy_pattern_drag!=PUPPY_RING_ANGLE {return puppy_title(project,puppy_pattern_hint(project));}
    let updated=puppy_pattern_drag_edit(project,x,y);if !updated {return puppy_title(project,cg_error);}
    puppy_pattern_preview=puppy_snapshot(j_encode(updated));return puppy_title(updated,"Dragging pattern");
}
fn puppy_pattern_event(event) {
    if !puppy_patterns {return 0;}let kind=gui_event_kind(event);let x=gui_event_x(event);let y=gui_event_y(event);
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 {
        let current=cad_project_open(puppy_history+24);let w=gui_width(load64(puppy_device));
        if y>=42 && y<68 {if w<800 {puppy_compact_bar(0,w,current,x);}else {puppy_action_bar(puppy_pattern_actions(current),0,w,x);}}
        else if y>=76 {puppy_pattern_down(current,x,y);}return 1;
    }
    if !puppy_pattern_drag {return 0;}let current=cad_project_open(puppy_history+24);
    if kind==GUI_MOUSE_MOVE {return puppy_pattern_motion(current,x,y);}
    if kind==GUI_MOUSE_UP && gui_event_button(event)==1 {
        puppy_pattern_motion(current,x,y);
        if puppy_pattern_preview {
            let updated=cad_project_open(puppy_pattern_preview+24);puppy_pattern_accept(current,updated);
            puppy_pattern_clear_preview();
        }
        puppy_pattern_drag=0;return 1;
    }
    if kind==GUI_FOCUS && !gui_event_code(event) {return puppy_pattern_cancel(current);}
    if kind==GUI_WHEEL {return 1;}return 0;
}
