import "smooth.flex";

// Draft coordinates use 4 logical pixels per millimetre at the default zoom.
global puppy_draft=0;
global puppy_draft_index=0;
global puppy_profile_index=1;
global puppy_tool=0;
global puppy_offset_pick_id=0;
global puppy_offset_editing=0;
global puppy_curves_page=0;
global puppy_revolve_axis=0;
global puppy_curve_coords=0;
global puppy_anchor=0;
global puppy_ax=0;
global puppy_ay=0;
global puppy_mx=0;
global puppy_my=0;
// Thousandths of a logical pixel per mm permit large drawings on narrow windows.
global puppy_draft_scale=4000;
global puppy_draft_cx=0;
global puppy_draft_cy=0;

// The picker borrows the sketch canvas. The saved view is restored on cancel.
global puppy_loft_open=0;
global puppy_loft_first=0;
global puppy_loft_profile=0;
global puppy_loft_view=0;
global puppy_face_operation=0;
fn puppy_pending_face_index(project) {
    let part=cad_project_part(project);let nodes=pc_nodes(part);
    let entry=pc_first(j_get(j_get(j_get(part,"data"),"tree"),"orderedNodeIds"));
    if !entry {return -1;}while load64(entry+24) {entry=load64(entry+24);}
    if j_kind(entry)!=2 {return -1;}
    let last=pc_find(nodes,j_value(entry));
    if !m_eq(j_string(last,"type"),"sketch") || !m_eq(j_string(pc_find(nodes,j_string(last,"targetId")),"type"),"face") {return -1;}
    let row=pc_first(nodes);let index=0;
    while row && row!=last {if m_eq(j_string(row,"type"),"sketch") {index=index+1;}row=load64(row+24);}
    return index;
}
fn puppy_pending_face(project) {
    let index=puppy_pending_face_index(project);if index<0 {return 0;}
    puppy_draft_index=index;puppy_profile_index=1;puppy_anchor=0;puppy_face_operation=0;return 1;
}
fn puppy_draft_on_face(project) {
    let sketch=puppy_draft_sketch(project);
    return m_eq(j_string(pc_find(pc_nodes(cad_project_part(project)),j_string(sketch,"targetId")),"type"),"face");
}
fn puppy_operation(project) {
    if puppy_draft_on_face(project) {
        if !puppy_face_operation {return "join";}if puppy_face_operation==1 {return "cut";}
    }
    return "newBody";
}
fn puppy_draft_sketch(project) {
    let row=pc_first(pc_nodes(cad_project_part(project)));let first=0;let index=0;
    while row {
        if m_eq(j_string(row,"type"),"sketch") {
            if !first {first=row;}if index==puppy_draft_index {return row;}index=index+1;
        }
        row=load64(row+24);
    }
    puppy_draft_index=0;return first;
}
fn puppy_fit_sketch(project,only_outside) {
    let sketch=puppy_draft_sketch(project);
    let paths=cad_sketch_paths(pc_nodes(cad_project_part(project)),j_string(sketch,"id"));if !paths {return 0;}
    if !puppy_reader_report && puppy_draft_on_face(project) {
        let guides=cad_project_sketch_face_loops(project,sketch);let i=0;
        while i<cg_count(guides) {cg_push(paths,cg_v(cg_at(guides,i),1,0));i=i+1;}
    }
    let low=cg_v(500000001,500000001,0);let high=cg_v(-500000001,-500000001,0);let i=0;
    while i<cg_count(paths) {
        let points=load64(cg_at(paths,i));let j=0;while j<cg_count(points) {
            let p=cg_at(points,j);let k=0;while k<16 {
                let n=load64(p+k);if n<load64(low+k) {store64(low+k,n);}if n>load64(high+k) {store64(high+k,n);}k=k+8;
            }
            j=j+1;
        }
        i=i+1;
    }
    if load64(high)<load64(low) {return 1;}
    let window=load64(puppy_device);let w=gui_width(window)-48;let h=gui_height(window)-76-48;
    if w<1 || h<1 {return 1;}
    let l=load64(low);let r=load64(high);let b=load64(low+8);let t=load64(high+8);
    let half_w=w*500000000/puppy_draft_scale;let half_h=h*500000000/puppy_draft_scale;
    if only_outside && l>=puppy_draft_cx-half_w && r<=puppy_draft_cx+half_w
        && b>=puppy_draft_cy-half_h && t<=puppy_draft_cy+half_h {return 1;}
    let scale=32000;
    if r>l {scale=puppy_limit(w*1000000000/(r-l),1,scale);}
    if t>b {scale=puppy_limit(h*1000000000/(t-b),1,scale);}
    puppy_draft_scale=scale;puppy_draft_cx=(l+r)/2;puppy_draft_cy=(b+t)/2;return 1;
}
fn puppy_draft_point(x,y) {
    let window=load64(puppy_device);
    return cg_v(puppy_draft_cx+(x-gui_width(window)/2)*1000000000/puppy_draft_scale,
        puppy_draft_cy+((gui_height(window)+76)/2-y)*1000000000/puppy_draft_scale,0);
}
fn puppy_tool_name() {
    if puppy_tool==-1 {return "point";}if puppy_tool==-2 {return "constructionPoint";}
    if puppy_tool==3 {return "arc";}if puppy_tool==4 {return "fit";}
    if puppy_tool==5 {return "control";}if puppy_tool==6 {return "periodic";}
    if puppy_tool==2 {return "line";}
    if puppy_tool==1 {return "circle";}
    return "rectangle";
}
fn puppy_draft_hint() {
    if puppy_tool==-4 {if puppy_anchor {return "Click the new tangent arc endpoint";}return "Pick a curve endpoint to start a tangent arc";}
    if puppy_tool==-5 {if puppy_anchor {return "Pick the curve that should follow or drag a source point";}return "Pick the source curve for Smooth";}
    if puppy_tool==-3 && !puppy_revolve_axis {return "Pick a source to offset or a copy to resize";}
    if puppy_revolve_axis {
        if puppy_anchor {return "Click the axis endpoint";}
        return "Click two axis points or choose X or Y";
    }
    if puppy_tool==-1 {return "Click to place a sketch point";}
    if puppy_tool==-2 {return "Click to place a construction point";}
    if puppy_tool==3 {
        if !puppy_anchor {return "Click the arc start";}if puppy_anchor==1 {return "Click a point on the arc";}return "Click the arc end";
    }
    if puppy_tool>=4 {
        if puppy_tool==5 {return "Control spline: pick 3n + 1 points, then Finish";}
        if puppy_tool==6 {return "Periodic spline: pick at least three points, then Finish";}
        return "Fit spline: pick at least two points, then Finish";
    }
    if puppy_tool==2 {return "Click the two endpoints";}
    if puppy_tool==1 {return "Click the centre, then the rim";}
    return "Click two rectangle corners";
}
fn puppy_curve_input(hover) {
    let points=cg_array();let i=0;
    while i<puppy_anchor {
        let pair=puppy_curve_coords+i*16;cg_push(points,cg_v(load64(pair),load64(pair+8),0));i=i+1;
    }
    if hover && i && i<4096 {
        let last=cg_at(points,i-1);
        if load64(last)!=puppy_mx || load64(last+8)!=puppy_my {cg_push(points,cg_v(puppy_mx,puppy_my,0));}
    }
    return points;
}
fn puppy_curve_finish(project) {
    let sketch=puppy_draft_sketch(project);let points=puppy_curve_input(0);
    if !cad_project_add_curve(project,j_string(sketch,"id"),puppy_tool_name(),points) {return puppy_title(project,cg_error);}
    puppy_anchor=0;puppy_push(project);return puppy_title(project,"Curve added");
}
fn puppy_sketch_hit(nodes,sk,point) {
    let paths=cad_sketch_paths(nodes,sk);if !paths {return 0;}
    let best=10000000000/puppy_draft_scale+1;let hit=0;let i=0;while i<cg_count(paths) {
        let path=cg_at(paths,i);let pts=load64(path);let count=cg_count(pts);let j=0;
        if count==1 {let d=pc_line_span(point,cg_at(pts,0));if d<best {best=d;hit=load64(path+16);}}
        while j+1<count+load64(path+8) {
            let d=puppy_segment_distance(point,cg_at(pts,j),cg_at(pts,(j+1)%count));
            if d<best {best=d;hit=load64(path+16);}j=j+1;
        }
        i=i+1;
    }
    return hit;
}
fn puppy_offset_pick(project,point) {
    let sketch=puppy_draft_sketch(project);let sk=j_string(sketch,"id");let nodes=pc_nodes(cad_project_part(project));
    let hit=puppy_sketch_hit(nodes,sk,point);
    if !hit {return puppy_title(project,puppy_draft_hint());}
    let id=j_string(hit,"id");let records=pc_line_records(nodes,sketch);if !records {return puppy_title(project,cg_error);}
    let rel=po_owner(records,id);puppy_offset_editing=rel!=0;if rel {id=j_string(rel,"id");}
    else if !(po_profile_node(hit) || pc_sketch_circle(hit) || pc_circular_arc(hit) || m_eq(j_string(hit,"type"),"sketchLine")) {
        return puppy_title(project,"Select a line, circle, circular arc, rectangle, polygon or slot");
    }
    puppy_offset_pick_id=puppy_snapshot(id);return puppy_number_open(project,puppy_active_field(project));
}
fn puppy_draft_click(x,y) {
    let project=cad_project_open(puppy_history+24);let p=puppy_draft_point(x,y);
    puppy_mx=load64(p);puppy_my=load64(p+8);
    if puppy_revolve_axis {
        if puppy_anchor {return puppy_draft_revolve(project,cg_v(puppy_ax,puppy_ay,0),p);}
        puppy_anchor=1;puppy_ax=puppy_mx;puppy_ay=puppy_my;
        return puppy_title(project,puppy_draft_hint());
    }
    if puppy_tool==-4 || puppy_tool==-5 {return puppy_join_click(project,p);}
    if puppy_tool==-3 {return puppy_offset_pick(project,p);}
    if puppy_tool<0 {
        let sketch=puppy_draft_sketch(project);
        if !cad_project_add_primitive(project,j_string(sketch,"id"),puppy_tool_name(),p,0) {return puppy_title(project,cg_error);}
        puppy_push(project);return puppy_title(project,"Point added");
    }
    if puppy_tool>=3 {
        if puppy_anchor>=4096 {return puppy_title(project,"Finish the curve before adding more points");}
        let pair=puppy_curve_coords+puppy_anchor*16;
        store64(pair,puppy_mx);store64(pair+8,puppy_my);puppy_anchor=puppy_anchor+1;
        if puppy_tool==3 && puppy_anchor==3 {
            let status=puppy_curve_finish(project);if puppy_anchor {puppy_anchor=2;}return status;
        }
        return puppy_title(project,puppy_draft_hint());
    }
    if !puppy_anchor {
        puppy_anchor=1;puppy_ax=puppy_mx;puppy_ay=puppy_my;
        if puppy_tool==2 {return puppy_title(project,"Click the line endpoint");}
        if puppy_tool==1 {return puppy_title(project,"Click the rim");}
        return puppy_title(project,"Click the opposite corner");
    }
    let sketch=puppy_draft_sketch(project);
    if !cad_project_add_primitive(project,j_string(sketch,"id"),puppy_tool_name(),cg_v(puppy_ax,puppy_ay,0),p) {
        return puppy_title(project,cg_error);
    }
    puppy_anchor=0;puppy_push(project);return puppy_title(project,"Sketch edited");
}
fn puppy_draft_extrude(project) {
    let sketch=puppy_draft_sketch(project);
    let updated=cad_project_add_extrusion(project,j_string(sketch,"id"),puppy_profile_index,10000000,puppy_operation(project));
    return puppy_finish_feature(project,updated,"Extruded");
}
fn puppy_draft_revolve(project,a,b) {
    let sketch=puppy_draft_sketch(project);
    let updated=cad_project_add_revolve(project,j_string(sketch,"id"),puppy_profile_index,a,b,360000000,puppy_operation(project));
    if !updated {puppy_anchor=0;return puppy_title(project,cg_error);}
    return puppy_finish_feature(project,updated,"Revolved");
}
fn puppy_finish_feature(project,updated,status) {
    if !updated {return puppy_title(project,cg_error);}
    if !puppy_refresh(updated) {return puppy_title(project,"Cannot draw feature");}
    puppy_revolve_axis=0;puppy_anchor=0;
    let feature=pc_first(pc_nodes(cad_project_part(updated)));let count=0;
    while feature {if puppy_driving_feature(feature) {count=count+1;}feature=load64(feature+24);}
    puppy_selected=count-1;puppy_property=0;puppy_push(updated);return puppy_title(updated,status);
}
fn puppy_loft_close() {
    if !puppy_loft_open {return 0;}
    let view=puppy_loft_view;
    puppy_draft=load64(view);puppy_sketch=load64(view+8);puppy_draft_index=load64(view+16);
    puppy_profile_index=load64(view+24);puppy_draft_cx=load64(view+32);puppy_draft_cy=load64(view+40);puppy_draft_scale=load64(view+48);
    if puppy_loft_first {puppy_drop(puppy_loft_first);puppy_loft_first=0;}
    puppy_loft_open=0;return 1;
}
fn puppy_loft_prompt(project) {
    let stage="First";if puppy_loft_first {stage="Second";}
    return puppy_title(project,m_cat(stage,m_cat(" loft profile ",m_int(puppy_profile_index))));
}
fn puppy_loft_begin(project) {
    puppy_search_focus(0);puppy_revolve_axis=0;puppy_anchor=0;
    let view=puppy_loft_view;
    store64(view,puppy_draft);store64(view+8,puppy_sketch);store64(view+16,puppy_draft_index);
    store64(view+24,puppy_profile_index);store64(view+32,puppy_draft_cx);store64(view+40,puppy_draft_cy);store64(view+48,puppy_draft_scale);
    puppy_loft_open=1;puppy_draft=1;puppy_sketch=1;
    puppy_fit_sketch(project,0);return puppy_loft_prompt(project);
}
fn puppy_loft_key(project,key,mods) {
    if key==GUI_KEY_ESCAPE {puppy_loft_close();return puppy_title(project,"Loft cancelled");}
    if key==43 || key==19 {
        puppy_draft_key(project,key,0);if key==43 {puppy_fit_sketch(project,0);}
        return puppy_loft_prompt(project);
    }
    if key!=40 && key!=38 {return 1;}
    let sketch=puppy_draft_sketch(project);let id=j_string(sketch,"id");
    if !puppy_loft_first {
        let ref=pc_profile_ref(id,puppy_profile_index);let region=cad_project_profile(pc_nodes(cad_project_part(project)),sketch,ref);
        if !region {return puppy_title(project,cg_error);}
        if cg_count(region)!=1 {return puppy_title(project,"Loft profiles cannot have holes");}
        puppy_loft_first=puppy_snapshot(id);puppy_loft_profile=puppy_profile_index;
        puppy_draft_key(project,43,0);puppy_fit_sketch(project,0);return puppy_loft_prompt(project);
    }
    let operation="newBody";if mods&3 {operation="join";}
    let updated=cad_project_add_loft(project,puppy_loft_first+24,puppy_loft_profile,id,puppy_profile_index,operation);
    if !updated {return puppy_title(project,cg_error);}
    puppy_loft_close();
    return puppy_finish_feature(project,updated,"Loft created");
}
fn puppy_draft_key(project,key,mods) {
    if key==37 {
        puppy_revolve_axis=!puppy_revolve_axis;puppy_anchor=0;return puppy_title(project,puppy_draft_hint());
    }
    if puppy_revolve_axis {
        if key==27 {return puppy_draft_revolve(project,cg_v(0,0,0),cg_v(1000000,0,0));}
        if key==28 {return puppy_draft_revolve(project,cg_v(0,0,0),cg_v(0,1000000,0));}
        return 1;
    }
    if key==13 {
        puppy_join_select(0);puppy_anchor=0;puppy_curves_page=1;puppy_tool=-5;if mods&3 {puppy_tool=-4;}
        return puppy_title(project,puppy_draft_hint());
    }
    if key==61 {puppy_curves_page=1;puppy_tool=-3;puppy_anchor=0;return puppy_title(project,puppy_draft_hint());}
    if key==74 {
        if !puppy_fit_sketch(project,0) {return puppy_title(project,cg_error);}
        return puppy_title(project,"Sketch fitted");
    }
    if key==10 {puppy_curves_page=!puppy_curves_page;return puppy_title(project,puppy_draft_hint());}
    if key==55 {
        puppy_anchor=0;puppy_curves_page=1;puppy_tool=-1;
        if mods&3 {puppy_tool=-2;}return puppy_title(project,puppy_draft_hint());
    }
    if key==4 || key==22 || key==5 || key==14 {
        puppy_anchor=0;puppy_curves_page=1;puppy_tool=3;
        if key==22 {puppy_tool=4;}if key==5 {puppy_tool=5;}if key==14 {puppy_tool=6;}
        return puppy_title(project,puppy_draft_hint());
    }
    if key==42 && puppy_anchor && puppy_tool>=3 {puppy_anchor=puppy_anchor-1;return puppy_title(project,puppy_draft_hint());}
    if key==40 && puppy_tool>=4 {return puppy_curve_finish(project);}
    if key==21 || key==6 || key==15 {
        puppy_curves_page=0;
        puppy_anchor=0;puppy_tool=key==6;
        if key==15 {puppy_tool=2;}
        return puppy_title(project,puppy_draft_hint());
    }
    if key==18 && puppy_draft_on_face(project) {
        puppy_face_operation=(puppy_face_operation+1)%3;return puppy_title(project,puppy_operation(project));
    }
    if key==8 {return puppy_draft_extrude(project);}
    if key==43 {
        let previous=puppy_draft_index;puppy_draft_index=previous+1;
        let sketch=puppy_draft_sketch(project);
        if !cad_sketch_paths(pc_nodes(cad_project_part(project)),j_string(sketch,"id")) {
            puppy_draft_index=previous;return puppy_title(project,cg_error);
        }
        puppy_profile_index=1;puppy_anchor=0;return puppy_title(project,puppy_draft_hint());
    }
    if key==19 {
        let probe=j_object();let sketch=puppy_draft_sketch(project);let id=j_string(sketch,"id");
        cg_j_text(probe,"profileId",m_cat(id,m_cat("-profile-",m_int(puppy_profile_index+1))));
        if cad_project_profile(pc_nodes(cad_project_part(project)),sketch,probe) {puppy_profile_index=puppy_profile_index+1;}
        else {puppy_profile_index=1;}
        return puppy_title(project,m_cat("Profile ",m_int(puppy_profile_index)));
    }
    return 1;
}
fn puppy_draft_path(points,mode,w,h) {
    let count=cg_count(points);let data=m_take(count*8);let i=0;
    while i<count {
        let p=cg_at(points,i);
        render_float(data+i*8,2*(load64(p)-puppy_draft_cx)*puppy_draft_scale,w*1000000000);
        render_float(data+i*8+4,2*(load64(p+8)-puppy_draft_cy)*puppy_draft_scale-76000000000,h*1000000000);i=i+1;
    }
    return render_mesh_upload(puppy_sketch_mesh,data,count,0x88e8)
        && render_draw(puppy_device,load64(puppy_sketch_mesh+8),mode,0,count);
}
fn puppy_draw_draft() {
    let window=load64(puppy_device);let w=gui_width(window);let h=gui_height(window);
    if !w || !h {return 1;}
    let source=puppy_history;if puppy_line_preview {source=puppy_line_preview;}if puppy_pattern_preview {source=puppy_pattern_preview;}if puppy_join_preview_doc {source=puppy_join_preview_doc;}
    let project=cad_project_open(source+24);let sketch=puppy_draft_sketch(project);let id=j_string(sketch,"id");
    if !puppy_revolve_axis && puppy_tool>=0 && puppy_tool<3 && puppy_anchor && (puppy_ax!=puppy_mx || puppy_ay!=puppy_my) {
        cad_project_add_primitive(project,id,puppy_tool_name(),cg_v(puppy_ax,puppy_ay,0),cg_v(puppy_mx,puppy_my,0));cg_begin();
    }
    let paths=cad_sketch_paths(pc_nodes(cad_project_part(project)),id);
    if !paths {return 0;}
    render_gl(puppy_device,"glUseProgram",puppy_sketch_program,0,0,0,0,0);
    render_gl(puppy_device,"glDisable",0x0b71,0,0,0,0,0);
    let axes=cg_array();let extent=8000000000;
    cg_push(axes,cg_v(-extent,0,0));cg_push(axes,cg_v(extent,0,0));
    cg_push(axes,cg_v(0,-extent,0));cg_push(axes,cg_v(0,extent,0));
    let color=render_gl(puppy_device,"glGetUniformLocation",puppy_sketch_program,"guide",0,0,0,0);
    render_gl(puppy_device,"glUniform1i",color,1,0,0,0,0);
    if !puppy_draft_path(axes,1,w,h) {return 0;}
    if !puppy_reader_report && puppy_draft_on_face(project) {
        let loops=cad_project_sketch_face_loops(project,sketch);let j=0;
        while j<cg_count(loops) {if !puppy_draft_path(cg_at(loops,j),2,w,h) {return 0;}j=j+1;}
    }
    if puppy_revolve_axis && puppy_anchor {
        let axis=cg_array();cg_push(axis,cg_v(puppy_ax,puppy_ay,0));cg_push(axis,cg_v(puppy_mx,puppy_my,0));
        if !puppy_draft_path(axis,1,w,h) {return 0;}
    }
    if !puppy_revolve_axis && puppy_anchor && puppy_tool>=3 {
        let pending=puppy_curve_input(1);let count=cg_count(pending);
        if !puppy_draft_path(pending,3,w,h) {return 0;}
        if puppy_tool==5 && count>1 {count=count-(count-1)%3;store64(pending,count);}
        if count>=2 {
            let curve=cad_project_add_curve(project,id,puppy_tool_name(),pending);
            if curve {
                let preview=pc_curve_path(curve);
                if preview {cg_push(paths,cg_v(preview,0,curve));}
            }
            cg_begin();
        }
    }
    if !puppy_revolve_axis && puppy_tool==-4 && puppy_anchor && puppy_join_source {
        let preview=puppy_join_preview(project,id);if preview {if !puppy_draft_path(preview,3,w,h) {return 0;}}cg_begin();
    }
    let selected=0;if puppy_tool==-5 && puppy_anchor && puppy_join_source {selected=j_string(j_parse(puppy_join_source+24),"entityId");}
    let offset=0;if puppy_line_report && !puppy_distance_row() {
        let row=puppy_line_row();selected=j_string(row,"id");if po_kind(row) && !puppy_symmetry_mode {offset=row;}
    }
    if puppy_symmetry_mode {selected=0;if puppy_symmetry_axis {selected=puppy_symmetry_axis+24;}}
    if puppy_drag_point_id {selected=puppy_drag_point_id+24;}
    let pattern=0;let sources=0;if puppy_patterns {pattern=puppy_pattern_rel(project);if puppy_pattern_sources {sources=j_parse(puppy_pattern_sources+24);}}
    let i=0;
    while i<cg_count(paths) {
        let path=cg_at(paths,i);let points=load64(path);let mode=3;
        if load64(path+8) {mode=2;}else if cg_count(points)==1 {mode=0;}
        let entity=load64(path+16);let highlight=selected && m_eq(j_string(entity,"id"),selected);
        if offset && po_target(offset,j_string(entity,"id")) {highlight=1;}
        if pattern && puppy_pattern_contains(pattern,j_string(entity,"id")) {highlight=1;}
        let src=pc_first(sources);while src {if m_eq(j_value(src),j_string(entity,"id")) {highlight=1;}src=load64(src+24);}
        let style=puppy_loft_open;
        if mode==0 && j_value(j_get(j_get(entity,"primitive"),"construction")) {style=3;}
        if highlight {style=2;}
        render_gl(puppy_device,"glUniform1i",color,style,0,0,0,0);
        if mode==0 {render_gl(puppy_device,"glEnable",0x8642,0,0,0,0,0);}
        if !puppy_draft_path(points,mode,w,h) {return 0;}
        if mode==0 {render_gl(puppy_device,"glDisable",0x8642,0,0,0,0,0);}
        if highlight && puppy_line_report && !offset && pc_sketch_circle(entity) && j_get(puppy_line_row(),"field") {
            let circle=j_get(entity,"primitive");let c=pc_uv(j_get(circle,"center"));let r=pc_pose_number(j_get(circle,"radius"));
            let a=cg_v(load64(c),load64(c+8),0);if m_eq(j_string(puppy_line_row(),"field"),"diameter") {store64(a,load64(a)-r);}
            let guide=cg_array();cg_push(guide,a);cg_push(guide,cg_v(load64(c)+r,load64(c+8),0));
            if !puppy_draft_path(guide,1,w,h) {return 0;}
        }else if highlight && puppy_line_report && !offset && pc_finite_arc(entity) && j_get(puppy_line_row(),"field") {
            let c=pc_uv(j_get(pc_curve_geometry(entity),"center"));let guide=cg_array();
            cg_push(guide,c);cg_push(guide,cg_at(points,0));
            if m_eq(j_string(puppy_line_row(),"field"),"arcSweep") {
                cg_push(guide,c);cg_push(guide,cg_at(points,cg_count(points)-1));
            }
            if !puppy_draft_path(guide,1,w,h) {return 0;}
        }else if highlight && puppy_tool==-5 && puppy_anchor && puppy_join_source {
            let rows=puppy_join_handles(entity);let handles=cg_array();let k=0;
            while k<cg_count(rows) {cg_push(handles,load64(cg_at(rows,k)));k=k+1;}
            render_gl(puppy_device,"glUniform1i",color,5,0,0,0,0);render_gl(puppy_device,"glEnable",0x8642,0,0,0,0,0);
            if !puppy_draft_path(handles,0,w,h) {return 0;}render_gl(puppy_device,"glDisable",0x8642,0,0,0,0,0);
        }else if highlight && !pattern {
            render_gl(puppy_device,"glEnable",0x8642,0,0,0,0,0);
            if !puppy_draft_path(points,0,w,h) {return 0;}
            render_gl(puppy_device,"glDisable",0x8642,0,0,0,0,0);
        }
        i=i+1;
    }
    if pattern && !puppy_pattern_guides(project,pattern,color,w,h) {return 0;}
    let distance=puppy_distance_row();
    let first=puppy_distance_first;if puppy_symmetry_mode {first=puppy_symmetry_anchor;}
    if first {
        let anchor=pc_saved_anchor(pc_nodes(cad_project_part(project)),id,j_parse(first+24));
        if anchor {
            let pick=cg_array();cg_push(pick,pc_uv(anchor));render_gl(puppy_device,"glUniform1i",color,2,0,0,0,0);
            render_gl(puppy_device,"glEnable",0x8642,0,0,0,0,0);puppy_draft_path(pick,0,w,h);render_gl(puppy_device,"glDisable",0x8642,0,0,0,0,0);
        }
    }
    if distance && !puppy_distance_picking && !puppy_symmetry_mode && !puppy_line_preview {
        let a=pc_uv(j_get(distance,"from"));let b=pc_uv(j_get(distance,"to"));let path=cg_array();cg_push(path,a);
        let axis=pc_distance_axis(distance);let mid=b;
        if axis==1 {mid=cg_v(load64(b),load64(a+8),0);}if axis==2 {mid=cg_v(load64(a),load64(b+8),0);}
        cg_push(path,mid);render_gl(puppy_device,"glUniform1i",color,2,0,0,0,0);
        puppy_draft_path(path,1,w,h);
        if axis {let guide=cg_array();cg_push(guide,mid);cg_push(guide,b);render_gl(puppy_device,"glUniform1i",color,1,0,0,0,0);puppy_draft_path(guide,1,w,h);}
        let ends=cg_array();cg_push(ends,a);cg_push(ends,b);render_gl(puppy_device,"glUniform1i",color,2,0,0,0,0);
        render_gl(puppy_device,"glEnable",0x8642,0,0,0,0,0);puppy_draft_path(ends,0,w,h);render_gl(puppy_device,"glDisable",0x8642,0,0,0,0,0);
    }
    if puppy_loft_open {
        render_gl(puppy_device,"glUniform1i",color,0,0,0,0,0);
        let region=cad_project_profile(pc_nodes(cad_project_part(project)),sketch,pc_profile_ref(id,puppy_profile_index));
        if region {
            i=0;while i<cg_count(region) {
                if !puppy_draft_path(load64(cg_at(region,i)),2,w,h) {return 0;}i=i+1;
            }
        }
    }
    return 1;
}
