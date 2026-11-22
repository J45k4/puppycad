// The report and drag preview outlive an event's arena. Only an accepted
// dimension or mouse release pushes a document snapshot onto undo history.
import "symmetry.flex";
import "line-directions.flex";
import "cad/project-offset-author.flex";

global puppy_line_report=0;
global puppy_line_index=0;
global puppy_new_offset=0;
global puppy_distance_index=-1;
global puppy_distance_picking=0;
global puppy_distance_first=0;
global puppy_distance_axis=0;
global puppy_distance_reference=0;
global puppy_line_was_draft=0;
global puppy_line_was_sketch=0;
global puppy_line_old_sketch=0;
global puppy_line_drag=0;
global puppy_drag_point_id=0;
global puppy_line_preview=0;
global puppy_line_down_x=0;
global puppy_line_down_y=0;

fn puppy_line_row() {
    let report=j_parse(puppy_line_report+24);let lines=j_get(report,"lines");let radial=j_get(report,"radial");
    let offsets=j_get(report,"offsets");let n=cg_j_count(lines);let end=n+cg_j_count(radial);let total=end+cg_j_count(offsets);if !total {return 0;}
    puppy_line_index=puppy_line_index%total;if puppy_line_index<n {return sr_arg(lines,puppy_line_index);}
    if puppy_line_index<end {return sr_arg(radial,puppy_line_index-n);}
    return sr_arg(offsets,puppy_line_index-end);
}
fn puppy_offset_field(row) {
    return row && !po_kind(row) && (j_get(row,"length") || m_eq(j_string(row,"field"),"radius") || m_eq(j_string(row,"field"),"diameter"));
}
fn puppy_curve_label(row) {
    if po_kind(row) {return "Offset";}
    let label="Diameter";if m_eq(j_string(row,"field"),"radius") {label="Radius";}
    let field=j_string(row,"field");
    if m_eq(field,"arcSweep") {label="Sweep";}if m_eq(field,"rotation") {label="Rotation";}
    if m_eq(field,"width") {label="Width";}if m_eq(field,"height") {label="Height";}
    if j_value(j_get(row,"reference")) {return m_cat("Reference ",label);}return label;
}
fn puppy_distance_row() {
    if puppy_distance_index<0 || !puppy_line_report {return 0;}
    let rows=j_get(j_parse(puppy_line_report+24),"distances");let count=cg_j_count(rows);
    if !count {puppy_distance_index=-1;return 0;}puppy_distance_index=puppy_distance_index%count;
    let row=pc_first(rows);let i=0;while i<puppy_distance_index {row=load64(row+24);i=i+1;}return row;
}
fn puppy_distance_name(row) {
    let axis=pc_distance_axis(row);let name="Distance";if axis==1 {name="X distance";}if axis==2 {name="Y distance";}
    if j_value(j_get(row,"reference")) {name=m_cat("Reference ",name);}return name;
}
fn puppy_line_label() {
    if puppy_direction_mode {return puppy_direction_hint();}
    if puppy_symmetry_mode {return puppy_symmetry_hint();}
    let row=puppy_line_row();if !row && !puppy_distance_row() {return "Choose New distance to measure sketch points";}let value="";if row {
        if j_get(row,"field") {value=m_cat(puppy_curve_label(row),m_cat(" ",puppy_display_number(pc_field_number(j_get(row,"measured"),j_string(row,"field")))));}
        else {value=puppy_display_number(pc_pose_number(j_get(row,"length")));}
    }
    let distance=puppy_distance_row();
    if distance {
        value=puppy_display_number(pc_serial_number(j_get(distance,"measured"),6,1000000,1500));
        value=m_cat(puppy_distance_name(distance),m_cat("   ",value));row=distance;
    }
    let unit=" mm";if pc_angular_field(j_string(row,"field")) {unit=" deg";}
    let report=j_parse(puppy_line_report+24);let text=m_cat(j_string(row,"id"),m_cat("   ",m_cat(value,unit)));
    let dof="   DOF ";if j_value(j_get(report,"degrees_of_freedom_is_upper_bound")) {dof="   DOF <= ";}
    return m_cat(text,m_cat(dof,m_int(j_integer(report,"degrees_of_freedom"))));
}
fn puppy_line_cache(report) {
    puppy_direction_end();
    if puppy_line_report {puppy_drop(puppy_line_report);}puppy_line_report=puppy_snapshot(j_encode(report));
    puppy_sketch=1;puppy_draft=1;puppy_anchor=0;puppy_revolve_axis=0;return 1;
}
fn puppy_line_read(project,id) {
    let model=puppy_sketch_model(project,id);if !model {return 0;}
    return cad_project_line_sketch(model,id);
}
fn puppy_line_choose(project,next) {
    let row=pc_first(puppy_workspace_sketch_records(project));let sketches=cg_array();
    while row {if m_eq(j_string(row,"type"),"sketch") {cg_push(sketches,row);}row=load64(row+24);}
    let count=cg_count(sketches);let i=0;let start=puppy_draft_index;
    if !puppy_draft {
        let source=j_string(puppy_feature(project),"sketchId");
        while i<count && !m_eq(j_string(cg_at(sketches,i),"id"),source) {i=i+1;}
        if i<count {start=i;}
        i=0;
    }
    while i<count {
        let index=(start+next+i)%count;let sketch=cg_at(sketches,index);
        let report=puppy_line_read(project,j_string(sketch,"id"));
        if report {
            if !puppy_line_report {puppy_line_was_draft=puppy_draft;puppy_line_was_sketch=puppy_sketch;puppy_line_old_sketch=puppy_draft_index;}
            puppy_line_cache(report);puppy_draft_index=index;puppy_line_index=0;puppy_distance_index=-1;if !puppy_line_row() {puppy_distance_index=0;}puppy_bar_page=0;
            puppy_fit_sketch(project,0);return puppy_title(project,puppy_line_label());
        }
        i=i+1;
    }
    return puppy_title(project,"No editable sketch dimensions in this part");
}
fn puppy_line_cancel_drag() {
    if puppy_line_preview {puppy_drop(puppy_line_preview);puppy_line_preview=0;}
    if puppy_drag_point_id {puppy_drop(puppy_drag_point_id);puppy_drag_point_id=0;}
    puppy_line_drag=0;return 1;
}
fn puppy_line_end() {
    puppy_direction_end();
    puppy_symmetry_end();puppy_distance_cancel();puppy_line_cancel_drag();puppy_drop(puppy_line_report);puppy_line_report=0;
}
fn puppy_line_close(project) {
    puppy_line_end();
    puppy_draft_index=puppy_line_old_sketch;puppy_draft=puppy_line_was_draft;puppy_sketch=puppy_line_was_sketch;
    if !puppy_refresh(project) {return puppy_title(project,cg_error);}
    if puppy_line_was_draft && pc_legacy_solid_part(cad_project_part(project)) {
        puppy_draft=1;puppy_sketch=1;puppy_empty=0;puppy_explicit_sketch=1;
    }
    return puppy_title(project,"Line editing closed");
}
fn puppy_line_update(project,updated) {
    if !updated {return 0;}
    let id=j_string(puppy_draft_sketch(project),"id");
    if !pc_legacy_solid_part(cad_project_part(updated)) {updated=puppy_sketch_result(project,id,updated);}
    if !updated {return 0;}
    let report=puppy_line_read(updated,id);if !report {return 0;}
    puppy_line_cache(report);
    if !m_eq(j_encode(project),j_encode(updated)) {puppy_push(updated);}
    return updated;
}
fn puppy_line_actions() {
    let buttons=cg_array();
    if puppy_direction_mode {return puppy_direction_actions(buttons);}
    if puppy_symmetry_mode {
        puppy_compact_button(buttons,"Mirror copy",16,0);puppy_compact_button(buttons,"Pair anchors",19,0);
        puppy_compact_button(buttons,"Cancel",GUI_KEY_ESCAPE,0);return buttons;
    }
    if puppy_distance_picking {
        puppy_compact_button(buttons,"Direct",7,0);puppy_compact_button(buttons,"X distance",27,0);
        puppy_compact_button(buttons,"Y distance",28,0);let kind="Driving";if puppy_distance_reference {kind="Reference";}
        puppy_compact_button(buttons,kind,21,0);puppy_compact_button(buttons,"Cancel",GUI_KEY_ESCAPE,0);return buttons;
    }
    let distance=puppy_distance_row();let report=j_parse(puppy_line_report+24);
    if distance {
        if !j_value(j_get(distance,"reference")) {puppy_compact_button(buttons,"Set distance",59,0);}
        let mode="Set reference";if j_value(j_get(distance,"reference")) {mode="Set driving";}
        puppy_compact_button(buttons,mode,21,0);puppy_compact_button(buttons,"Next distance",7,0);
        if puppy_line_row() {
            let label="Select line";if pc_first(j_get(report,"radial")) {label="Select field";}
            puppy_compact_button(buttons,label,43,0);
        }
        else {puppy_compact_button(buttons,"Next sketch",19,0);}
        puppy_compact_button(buttons,"New distance",17,0);puppy_compact_button(buttons,"Remove",76,0);
    }else if po_kind(puppy_line_row()) {
        puppy_compact_button(buttons,"Set offset",59,0);puppy_compact_button(buttons,"Next field",43,0);
    }else if j_get(puppy_line_row(),"field") {
        let row=puppy_line_row();let label="Set diameter";if m_eq(j_string(row,"field"),"radius") {label="Set radius";}
        if m_eq(j_string(row,"field"),"arcSweep") {label="Set sweep";}
        if pc_axis_field(j_string(row,"field")) {label=m_cat("Set ",j_string(row,"field"));}
        if !j_value(j_get(row,"reference")) {puppy_compact_button(buttons,label,59,0);}
        puppy_compact_button(buttons,"Next field",43,0);puppy_compact_button(buttons,"Next sketch",19,0);
        label="Set reference";if j_value(j_get(row,"reference")) {label="Set driving";}
        puppy_compact_button(buttons,label,21,0);
        if j_get(row,"relationId") {puppy_compact_button(buttons,"Remove",76,0);}
        if pc_first(j_get(report,"distances")) {puppy_compact_button(buttons,"Distances",7,0);}
    }else if puppy_line_row() {
        let label="Next line";if pc_first(j_get(report,"radial")) {label="Next field";}
        puppy_compact_button(buttons,"Set length",59,0);puppy_compact_button(buttons,label,43,0);
        puppy_compact_button(buttons,"Next sketch",19,0);
        if pc_first(j_get(report,"distances")) {puppy_compact_button(buttons,"Distances",7,0);}
        else {puppy_compact_button(buttons,"Fit",74,0);}
    }else {
        puppy_compact_button(buttons,"New distance",17,0);puppy_compact_button(buttons,"Next sketch",19,0);puppy_compact_button(buttons,"Fit",74,0);
    }
    puppy_compact_button(buttons,"Variables",60,0);
    if !distance && puppy_line_row() {
        if pc_first(j_get(report,"distances")) {puppy_compact_button(buttons,"Fit",74,0);}
        else {puppy_compact_button(buttons,"New distance",17,0);}
    }
    if !distance && puppy_offset_field(puppy_line_row()) {puppy_compact_button(buttons,"New offset",61,0);}
    if pc_first(j_get(report,"offsets")) {puppy_compact_button(buttons,"Offsets",18,0);}
    if pc_first(j_get(report,"lines")) {puppy_compact_button(buttons,"Mirror",16,0);}
    puppy_compact_button(buttons,"Done",GUI_KEY_ESCAPE,0);return buttons;
}
fn puppy_distance_cancel() {
    if puppy_distance_first {puppy_drop(puppy_distance_first);puppy_distance_first=0;}
    puppy_distance_picking=0;return 1;
}
fn puppy_distance_hint(project) {
    let axis="Direct";if puppy_distance_axis==1 {axis="X";}if puppy_distance_axis==2 {axis="Y";}
    let kind=" driving distance";if puppy_distance_reference {kind=" reference distance";}
    let pick="   Pick first anchor";if puppy_distance_first {pick="   Pick second anchor";}
    return puppy_title(project,m_cat(axis,m_cat(kind,pick)));
}
fn puppy_distance_hit(point) {
    let report=j_parse(puppy_line_report+24);let best=10000000000/puppy_draft_scale+1;let hit=0;
    let group=0;while group<2 {
        let rows=j_get(report,"lines");if group {rows=j_get(report,"points");}
        let row=pc_first(rows);while row {
            let j=0;let count=2-group;
            while j<count {
                let end="p0";if j {end="p1";}if group {end="center";}
                let d=pc_line_span(point,pc_uv(j_get(row,end)));
                if d<best || (group && hit && d==best && !m_eq(j_string(hit,"point"),"center")) {hit=j_object();cg_j_text(hit,"entityId",j_string(row,"id"));cg_j_text(hit,"point",end);best=d;}
                j=j+1;
            }
            row=load64(row+24);
        }
        group=group+1;
    }
    return hit;
}
fn puppy_distance_pick(project,x,y) {
    let anchor=puppy_distance_hit(puppy_draft_point(x,y));if !anchor {return puppy_distance_hint(project);}
    if !puppy_distance_first {puppy_distance_first=puppy_snapshot(j_encode(anchor));return puppy_distance_hint(project);}
    let axis="";if puppy_distance_axis==1 {axis="x";}if puppy_distance_axis==2 {axis="y";}
    let id=j_string(puppy_draft_sketch(project),"id");let model=puppy_sketch_model(project,id);
    if !model {return puppy_title(project,cg_error);}
    let updated=puppy_line_update(project,cad_project_add_point_distance(model,id,j_parse(puppy_distance_first+24),anchor,axis,puppy_distance_reference));
    if !updated {return puppy_title(project,cg_error);}
    puppy_distance_cancel();puppy_distance_index=cg_j_count(j_get(j_parse(puppy_line_report+24),"distances"))-1;return puppy_title(updated,puppy_line_label());
}
fn puppy_line_command(project,key) {
    if puppy_direction_mode || key==15 && puppy_direction_available() {return puppy_direction_command(project,key);}
    if puppy_symmetry_mode {return puppy_symmetry_key(project,key);}
    if puppy_distance_picking {
        if key==GUI_KEY_ESCAPE {puppy_distance_cancel();return puppy_title(project,puppy_line_label());}
        if key==7 {puppy_distance_axis=0;}if key==27 {puppy_distance_axis=1;}if key==28 {puppy_distance_axis=2;}
        if key==21 {puppy_distance_reference=!puppy_distance_reference;}
        return puppy_distance_hint(project);
    }
    if key==GUI_KEY_ESCAPE && puppy_line_drag {puppy_line_cancel_drag();return puppy_title(project,"Drag cancelled");}
    if key==GUI_KEY_ESCAPE || key==65 {return puppy_line_close(project);}
    if puppy_line_drag {return 1;}
    if key==16 {return puppy_symmetry_key(project,key);}
    if key==17 {puppy_distance_picking=1;puppy_distance_axis=0;puppy_distance_reference=0;return puppy_distance_hint(project);}
    if key==7 {
        if pc_first(j_get(j_parse(puppy_line_report+24),"distances")) {puppy_distance_index=puppy_distance_index+1;return puppy_title(project,puppy_line_label());}return 1;
    }
    let distance=puppy_distance_row();
    let circle=puppy_line_row();
    if key==61 && !distance && puppy_offset_field(circle) {
        puppy_new_offset=1;return puppy_number_open(project,puppy_active_field(project));
    }
    if key==18 {
        let report=j_parse(puppy_line_report+24);let count=cg_j_count(j_get(report,"offsets"));if !count {return 1;}
        let start=cg_j_count(j_get(report,"lines"))+cg_j_count(j_get(report,"radial"));
        if !distance && puppy_line_index>=start {puppy_line_index=start+(puppy_line_index-start+1)%count;}else {puppy_line_index=start;}
        puppy_distance_index=-1;return puppy_title(project,puppy_line_label());
    }
    if !distance && po_kind(circle) && (key==21 || key==76) {return 1;}
    if !distance && j_get(circle,"field") && (key==21 || key==76) {
        let id=j_string(circle,"id");let field=j_string(circle,"field");let sketch=j_string(puppy_draft_sketch(project),"id");let updated=0;
        let model=puppy_sketch_model(project,sketch);if !model {return puppy_title(project,cg_error);}
        if key==21 {updated=cad_project_toggle_curve_reference(model,sketch,id,field);}
        else {updated=cad_project_remove_curve_dimension(model,sketch,id,field);}
        updated=puppy_line_update(project,updated);if !updated {return puppy_title(project,cg_error);}
        return puppy_title(updated,puppy_line_label());
    }
    if (key==76 || key==21) && distance {
        let id=j_string(puppy_draft_sketch(project),"id");let model=puppy_sketch_model(project,id);
        if !model {return puppy_title(project,cg_error);}
        let updated=0;
        if key==76 {updated=cad_project_remove_point_distance(model,id,j_string(distance,"id"));}
        else {updated=cad_project_toggle_distance_reference(model,id,j_string(distance,"id"));}
        updated=puppy_line_update(project,updated);if !updated {return puppy_title(project,cg_error);}
        return puppy_title(updated,puppy_line_label());
    }
    if key==59 {
        if distance && j_value(j_get(distance,"reference")) {return puppy_title(project,"Reference distance is measured");}
        if !distance && j_value(j_get(circle,"reference")) {
            let model=puppy_sketch_model(project,j_string(puppy_draft_sketch(project),"id"));
            let node=pc_find(pc_nodes(cad_project_part(model)),j_string(circle,"id"));
            if pc_axis_field(j_string(circle,"field")) {return puppy_title(project,"Reference ellipse dimension is measured");}
            if pc_finite_arc(node) {return puppy_title(project,"Reference arc dimension is measured");}
            return puppy_title(project,"Reference circle dimension is measured");
        }
        let field=puppy_active_field(project);if field {return puppy_number_open(project,field);}return 1;
    }
    if key==60 {return puppy_variable_open(project,0);}
    if key==19 {return puppy_line_choose(project,1);}
    if key==43 {
        if distance && puppy_line_row() {puppy_distance_index=-1;}
        else if distance {puppy_distance_index=puppy_distance_index+1;}
        else {puppy_line_index=puppy_line_index+1;}return puppy_title(project,puppy_line_label());}
    if key==74 {puppy_fit_sketch(project,0);}
    return 1;
}
fn puppy_line_down(project,x,y) {
    let point=puppy_draft_point(x,y);let tolerance=10000000000/puppy_draft_scale;
    let anchor=puppy_distance_hit(point);
    if anchor && m_eq(j_string(anchor,"point"),"center") {
        puppy_line_cancel_drag();puppy_drag_point_id=puppy_snapshot(j_string(anchor,"entityId"));
        puppy_line_drag=3;puppy_line_down_x=x;puppy_line_down_y=y;return puppy_title(project,"Drag sketch point");
    }
    let rows=j_get(j_parse(puppy_line_report+24),"lines");let row=pc_first(rows);let i=0;
    let best=tolerance+1;let selected=-1;let endpoint=0;
    while row {
        let a=pc_uv(j_get(row,"p0"));let b=pc_uv(j_get(row,"p1"));
        let da=pc_line_span(point,a);let db=pc_line_span(point,b);
        if da<=tolerance || db<=tolerance {
            let side=1;let d=da;if db<da {d=db;side=2;}
            if !endpoint || d<best {best=d;selected=i;endpoint=side;}
        }else if !endpoint {
            let dx=load64(b)-load64(a);let dy=load64(b+8)-load64(a+8);
            let px=load64(point)-load64(a);let py=load64(point+8)-load64(a+8);
            let dot=px*dx+py*dy;
            if dot>=0 && dot<=dx*dx+dy*dy {
                let d=cg_abs(px*dy-py*dx)/pc_line_span(a,b);if d<best {best=d;selected=i;}
            }
        }
        i=i+1;row=load64(row+24);
    }
    let report=j_parse(puppy_line_report+24);let dim=pc_first(j_get(report,"radial"));i=cg_j_count(rows);
    let nodes=0;if dim && !endpoint {nodes=pc_nodes(cad_project_part(puppy_sketch_model(project,j_string(puppy_draft_sketch(project),"id"))));}
    while dim && !endpoint {
        if m_eq(j_string(dim,"field"),"diameter") {
            let center=pc_find(j_get(report,"points"),j_string(dim,"id"));
            let d=cg_abs(pc_line_span(point,pc_uv(j_get(center,"center")))-pc_pose_number(j_get(center,"radius")));
            if d<best {best=d;selected=i;}
        }else {
            let node=pc_find(nodes,j_string(dim,"id"));
            let field=j_string(dim,"field");let closed=(pc_curve_axes(node) && !pc_finite_arc(node) && m_eq(field,"width")) || (pc_polygon_entity(node) && m_eq(field,"radius"));
            if closed || pc_finite_arc(node) && (m_eq(field,"radius") || !pc_circular_arc(node) && m_eq(field,"arcSweep")) {
                let path=0;if closed {path=pc_primitive_loop(node);}else {path=pc_arc_path(node);}
                let k=1;let n=cg_count(path);
                while n && k<n+closed {
                    let a=cg_at(path,k-1);let b=cg_at(path,k%n);let d=puppy_segment_distance(point,a,b);
                    if d<best {best=d;selected=i;}k=k+1;
                }
            }
        }
        i=i+1;dim=load64(dim+24);
    }
    if selected>=0 {
        puppy_distance_index=-1;puppy_line_index=selected;puppy_line_drag=endpoint;puppy_line_down_x=x;puppy_line_down_y=y;
        return puppy_title(project,puppy_line_label());
    }
    return 1;
}
fn puppy_line_move(project,x,y) {
    if puppy_line_preview {puppy_drop(puppy_line_preview);puppy_line_preview=0;}
    if x==puppy_line_down_x && y==puppy_line_down_y {return puppy_title(project,puppy_line_label());}
    let sketch=j_string(puppy_draft_sketch(project),"id");let at=pc_json_point(puppy_draft_point(x,y));
    let model=puppy_sketch_model(project,sketch);if !model {return puppy_title(project,cg_error);}
    let updated=0;let status="Dragging endpoint";
    if puppy_drag_point_id {
        updated=cad_project_move_point(model,sketch,puppy_drag_point_id+24,at);status="Dragging sketch point";
    }else {
        let end="p0";if puppy_line_drag==2 {end="p1";}
        updated=cad_project_move_line_endpoint(model,sketch,j_string(puppy_line_row(),"id"),end,at);
    }
    updated=puppy_sketch_result(project,sketch,updated);
    if !updated {return puppy_title(project,cg_error);}
    puppy_line_preview=puppy_snapshot(j_encode(updated));return puppy_title(project,status);
}
fn puppy_line_event(event) {
    let kind=gui_event_kind(event);if !puppy_line_report {return 0;}
    let x=gui_event_x(event);let y=gui_event_y(event);let project=cad_project_open(puppy_history+24);
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 {
        let w=gui_width(load64(puppy_device));
        if puppy_breadcrumb_click(project,x,y) {return 1;}
        if y>=7 && y<33 && x>=puppy_direction_button_left(w) && x<w-12 && puppy_direction_available() {return puppy_direction_command(project,15);}
        if y>=42 && y<68 {
            if w<800 {puppy_compact_bar(0,w,project,x);}else {puppy_action_bar(puppy_line_actions(),0,w,x);}
        }else if y>=76 {
            if puppy_direction_mode {puppy_direction_pick(project,x,y);}
            else if puppy_symmetry_mode {puppy_symmetry_pick(project,x,y);}
            else if puppy_distance_picking {puppy_distance_pick(project,x,y);}
            else {puppy_line_down(project,x,y);}
        }return 1;
    }
    if kind==GUI_MOUSE_MOVE && puppy_line_drag {return puppy_line_move(project,x,y);}
    if kind==GUI_MOUSE_UP && gui_event_button(event)==1 && puppy_line_drag {
        puppy_line_move(project,x,y);
        if puppy_line_preview {
            let updated=cad_project_open(puppy_line_preview+24);
            updated=puppy_line_update(project,updated);
            if updated {puppy_title(updated,puppy_line_label());}else {puppy_title(project,cg_error);}
        }
        return puppy_line_cancel_drag();
    }
    if kind==GUI_FOCUS && !gui_event_code(event) && puppy_line_drag {return puppy_line_cancel_drag();}
    return 0;
}
