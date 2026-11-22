import "cad/project-smooth-author.flex";

global puppy_join_source=0;
global puppy_join_drag=0;
global puppy_join_preview_doc=0;
global puppy_join_down_x=0;
global puppy_join_down_y=0;
global puppy_join_origin_x=0;
global puppy_join_origin_y=0;
fn puppy_join_select(anchor) {
    if puppy_join_source {puppy_drop(puppy_join_source);puppy_join_source=0;}
    if anchor {puppy_join_source=puppy_snapshot(j_encode(anchor));}
    return 1;
}
fn puppy_join_endpoint(nodes,sk,cursor) {
    let row=pc_first(nodes);let best=8000000000/puppy_draft_scale+1;let hit=0;
    while row {
        if m_eq(j_string(row,"sketchId"),sk) {
            let i=0;while i<2 {
                let name=ps_end_name(row,i);if name {
                    let frame=ps_endpoint(row,name);if frame {
                        let d=pc_line_span(cursor,load64(frame));if d<best {hit=ps_anchor(j_string(row,"id"),name);best=d;}
                    }else {cg_begin();}
                }i=i+1;
            }
        }row=load64(row+24);
    }return hit;
}
fn puppy_join_click(project,cursor) {
    let nodes=pc_nodes(cad_project_part(project));let sk=j_string(puppy_draft_sketch(project),"id");let hit=0;let updated=0;
    if puppy_tool==-4 {
        if puppy_anchor {
            updated=cad_project_add_tangent_arc(project,sk,j_parse(puppy_join_source+24),pc_json_point(cursor));
        }else {hit=puppy_join_endpoint(nodes,sk,cursor);}
    }else {
        let node=puppy_sketch_hit(nodes,sk,cursor);
        if node && ps_end_name(node,0) {
            hit=ps_anchor(j_string(node,"id"),ps_end_name(node,0));
            if puppy_anchor {updated=cad_project_toggle_smooth_curves(project,sk,j_string(j_parse(puppy_join_source+24),"entityId"),j_string(node,"id"));}
        }
    }
    if updated {
        puppy_join_select(0);puppy_anchor=0;puppy_push(updated);puppy_fit_sketch(updated,1);
        if puppy_tool==-4 {return puppy_title(updated,"Tangent arc added");}return puppy_title(updated,"Smooth join updated");
    }
    if cg_error {return puppy_title(project,cg_error);}
    if !puppy_anchor && hit {
        puppy_join_select(hit);puppy_anchor=1;puppy_ax=load64(cursor);puppy_ay=load64(cursor+8);
    }
    return puppy_title(project,puppy_draft_hint());
}
fn puppy_join_preview(project,id) {
    let nodes=pc_nodes(cad_project_part(project));let frame=ps_anchor_frame(nodes,id,j_parse(puppy_join_source+24));
    if !frame {return 0;}let arc=ps_tangent_arc("preview",frame,cg_v(puppy_mx,puppy_my,0));if !arc {return 0;}
    let node=j_object();cg_j_text(node,"type","sketchArc");j_add(node,"arc",arc);return pc_arc_path(node);
}

fn puppy_join_handles(node) {
    let rows=cg_array();let id=j_string(node,"id");
    if m_eq(j_string(node,"type"),"sketchSpline") {
        let p=pc_first(j_get(j_get(node,"spline"),"points"));let i=0;while p {
            cg_push(rows,cg_v(pc_uv(p),ps_anchor(id,m_cat("point",m_int(i))),0));p=load64(p+24);i=i+1;
        }
    }else {
        let i=0;while i<2 {
            let name=ps_end_name(node,i);if name {let f=ps_endpoint(node,name);if f {cg_push(rows,cg_v(load64(f),ps_anchor(id,name),0));}}
            i=i+1;
        }
        if pc_circular_arc(node) {cg_push(rows,cg_v(pc_uv(j_get(j_get(node,"arc"),"center")),ps_anchor(id,"center"),0));}
    }return rows;
}
fn puppy_join_clear_preview() {
    if puppy_join_preview_doc {puppy_drop(puppy_join_preview_doc);puppy_join_preview_doc=0;}return 1;
}
fn puppy_join_end_drag() {
    puppy_join_clear_preview();if puppy_join_drag {puppy_drop(puppy_join_drag);puppy_join_drag=0;}return 1;
}
fn puppy_join_event(event) {
    if !puppy_draft || puppy_tool!=-5 || !puppy_anchor || !puppy_join_source || puppy_line_report || puppy_patterns || puppy_loft_open {return 0;}
    let kind=gui_event_kind(event);let x=gui_event_x(event);let y=gui_event_y(event);
    if !puppy_join_drag && (kind!=GUI_MOUSE_DOWN || gui_event_button(event)!=1 || y<76) {return 0;}
    let project=cad_project_open(puppy_history+24);let sk=j_string(puppy_draft_sketch(project),"id");
    if !puppy_join_drag {
        let node=pc_find(pc_nodes(cad_project_part(project)),j_string(j_parse(puppy_join_source+24),"entityId"));
        let rows=puppy_join_handles(node);let at=puppy_draft_point(x,y);let i=0;let best=8000000000/puppy_draft_scale+1;let hit=0;
        while i<cg_count(rows) {let row=cg_at(rows,i);let d=pc_line_span(load64(row),at);if d<best {best=d;hit=row;}i=i+1;}
        if !hit {return 0;}puppy_join_drag=puppy_snapshot(j_encode(load64(hit+8)));puppy_join_down_x=x;puppy_join_down_y=y;
        puppy_join_origin_x=load64(load64(hit));puppy_join_origin_y=load64(load64(hit)+8);return 1;
    }
    if kind==GUI_FOCUS && !gui_event_code(event) {puppy_join_end_drag();return puppy_title(project,"Smooth drag cancelled");}
    let release=kind==GUI_MOUSE_UP && gui_event_button(event)==1;
    if kind!=GUI_MOUSE_MOVE && !release {if kind==GUI_WHEEL {return 1;}return 0;}
    puppy_join_clear_preview();let dx=x-puppy_join_down_x;let dy=puppy_join_down_y-y;
    if cg_abs(dx)<2 && cg_abs(dy)<2 {if release {puppy_join_end_drag();}return puppy_title(project,puppy_draft_hint());}
    let pt=cg_v(puppy_join_origin_x+dx*1000000000/puppy_draft_scale,puppy_join_origin_y+dy*1000000000/puppy_draft_scale,0);
    let changed=cad_project_move_smooth_point(project,sk,j_parse(puppy_join_drag+24),pc_json_point(pt));
    if !changed {if release {puppy_join_end_drag();}return puppy_title(project,cg_error);}
    if release {puppy_join_end_drag();puppy_push(changed);return puppy_title(changed,"Smooth point moved");}
    puppy_join_preview_doc=puppy_snapshot(j_encode(changed));return puppy_title(changed,"Dragging joined curve");
}
