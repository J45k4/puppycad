// Gestures own transient selection and camera state. Hinge previews always
// evaluate from the document at mouse-down. Mouse-up creates one undo entry.
global puppy_pick_id=0;
global puppy_first=0;
global puppy_last=0;
global puppy_pan=0;
global puppy_button=0;
global puppy_start_x=0;
global puppy_start_y=0;
global puppy_prev_x=0;
global puppy_prev_y=0;
global puppy_moved=0;
global puppy_hinge=0;
global puppy_rim=0;
global puppy_orbit_point=0;
global puppy_orbit_depth=0;
global puppy_arc=0;
global puppy_draw_ranges=0;
global puppy_range_bytes=0;
global puppy_home=0;
global puppy_angle=0;
global puppy_low=0;
global puppy_high=0;

global puppy_isolated=0;
global puppy_isolated_owner=0;
global puppy_visible_first=0;
global puppy_visible_count=0;
global puppy_body_count=0;
global puppy_click_id=0;
global puppy_click_time=0;
global puppy_click_x=0;
global puppy_click_y=0;

fn puppy_forget_click() {
    if puppy_click_id {puppy_drop(puppy_click_id);puppy_click_id=0;}
    return 1;
}
fn puppy_end_isolation() {
    if puppy_isolated {
        puppy_drop(puppy_isolated);puppy_isolated=0;
        puppy_drop(puppy_isolated_owner);puppy_isolated_owner=0;
    }
    return 1;
}
fn puppy_visible_id() {if puppy_isolated {return puppy_isolated+24;}return 0;}
fn puppy_isolate(project,id) {
    let mesh=cad_project_mesh(project);if j_get(mesh,"error") {return puppy_title(project,cg_error);}
    let part=0;if id {
        part=pc_find(j_get(mesh,"parts"),id);
        if !part || cg_j_count(j_get(mesh,"parts"))<2 {return 1;}
    }
    puppy_end_isolation();
    let message="Showing all parts";
    if part {
        puppy_isolated=puppy_snapshot(j_string(part,"id"));
        puppy_isolated_owner=puppy_snapshot(j_string(cad_project_item(project),"id"));
        message=m_cat("Isolated ",j_string(part,"name"));
    }
    if !puppy_upload(project,mesh) {return puppy_title(project,cg_error);}
    gui__zero(puppy_pan,24);puppy_zoom=280;
    return puppy_title(project,message);
}
fn puppy_occurrence_click() {
    let now=gui_ticks();let id=puppy_pick_id+24;
    let twice=puppy_click_id && m_eq(id,puppy_click_id+24) && now-puppy_click_time<=420
        && cg_abs(puppy_start_x-puppy_click_x)<=4 && cg_abs(puppy_start_y-puppy_click_y)<=4;
    puppy_forget_click();
    if twice {return puppy_isolate(cad_project_open(puppy_history+24),id);}
    puppy_click_id=puppy_snapshot(id);puppy_click_time=now;
    puppy_click_x=puppy_start_x;puppy_click_y=puppy_start_y;return 1;
}

fn puppy_view_up(forward) {
    return cg_v(load64(puppy_cam_basis+24),load64(puppy_cam_basis+32),load64(puppy_cam_basis+40));
}
fn puppy_ray_at(ray,depth) {
    let p=m_take(24);let k=0;
    while k<24 {store64(p+k,load64(load64(ray)+k)+load64(load64(ray+8)+k)*depth/1000);k=k+8;}
    return p;
}
fn puppy_clear_pick() {
    puppy_placement_open=0;puppy_placement_slot=-1;
    puppy_mate_selection(0);
    if puppy_mate_selection_frame {puppy_drop(puppy_mate_selection_frame);puppy_mate_selection_frame=0;}
    if puppy_definition {puppy_drop(puppy_definition);puppy_definition=0;}
    puppy_copy_focus=0;
    if puppy_pick_id {puppy_drop(puppy_pick_id);puppy_pick_id=0;}
    puppy_face_clear();puppy_vmenu_close();
    puppy_first=0;puppy_last=0;return 1;
}
fn puppy_limit(value,low,high) {if value<low {return low;}if value>high {return high;}return value;}
fn puppy_fit() {
    let project=cad_project_open(puppy_history+24);
    if cad_project_part(project) {puppy_part_sketch_fit();}
    gui__zero(puppy_pan,24);puppy_zoom=280;puppy_hover_clear();
    return puppy_title(project,"View fitted");
}
fn puppy_draw_occurrences() {
    let moving=render_uniform(puppy_material,0,"moving");
    render_gl(puppy_device,"glUniform1i",moving,0,0,0,0,0);
    if puppy_definition {
        let part=pc_first(j_parse(puppy_catalog+24));let uniform=render_uniform(puppy_material,0,"selection");
        while part {
            let start=j_integer(part,"first");let count=j_integer(part,"count");
            if !puppy_isolated || start==puppy_visible_first {
                let end=0;if m_eq(j_string(part,"part"),puppy_definition+24) {end=start+count;}
                render_gl(puppy_device,"glUniform2i",uniform,start,end,0,0,0);
                if !render_mesh_draw(puppy_surface,puppy_material,0,start,count) {return 0;}
            }
            part=load64(part+24);
        }
        return 1;
    }
    if !puppy_draw_ranges || puppy_angle==puppy_home {
        return render_mesh_draw(puppy_surface,puppy_material,0,puppy_visible_first,puppy_visible_count);
    }
    let axis=m_take(12);let pivot=m_take(12);let cs=m_take(8);let k=0;
    while k<3 {
        render_float(axis+k*4,load64(puppy_arc+72+k*8),1000000);
        render_float(pivot+k*4,load64(puppy_arc+k*8),1000000000);k=k+1;
    }
    let angle=(puppy_angle-puppy_home)/1000;
    render_float(cs,cg_cos(angle),1000000);render_float(cs+4,cg_sin(angle),1000000);
    render_gl(puppy_device,"glUniform3fv",render_uniform(puppy_material,0,"axis"),1,axis,0,0,0);
    render_gl(puppy_device,"glUniform3fv",render_uniform(puppy_material,0,"pivot"),1,pivot,0,0,0);
    render_gl(puppy_device,"glUniform2fv",render_uniform(puppy_material,0,"cs"),1,cs,0,0,0);
    let at=0;k=0;while k<puppy_range_bytes {
        let count=load64(puppy_draw_ranges+k);
        render_gl(puppy_device,"glUniform1i",moving,load64(puppy_draw_ranges+k+8),0,0,0,0);
        if (!puppy_isolated || at==puppy_visible_first) && !render_mesh_draw(puppy_surface,puppy_material,0,at,count) {return 0;}
        at=at+count;k=k+16;
    }
    return 1;
}
fn puppy_set_hinge(project,hit,parts) {
    let id=load64(load64(hit+16));let studio=cad_project_is_studio(project);let joint=0;
    let mesh=0;
    if studio {
        if !puppy_solved_assembly {return 0;}
        mesh=j_object();j_add(mesh,"assembly",j_parse(puppy_solved_assembly+24));joint=cad_studio_hinge(project,mesh,id);
    }else {joint=cad_project_servo_frame(project,id);}
    if !joint {return 0;}
    let frame=load64(joint);let servo=load64(joint+8);let mate=load64(joint+16);
    let point=load64(hit);let delta=cg_sub(point,frame+72);
    let axis=cg_v(load64(frame+16),load64(frame+40),load64(frame+64));
    m_copy(puppy_arc+72,axis,24);
    let along=(load64(delta)*load64(axis)+load64(delta+8)*load64(axis+8)+load64(delta+16)*load64(axis+16))/1000000;
    let k=0;let radius=0;
    while k<24 {
        let pivot=load64(frame+72+k)+load64(axis+k)*along/1000000;
        store64(puppy_arc+k,pivot);let r=load64(point+k)-pivot;
        store64(puppy_arc+24+k,r);radius=radius+r*r;k=k+8;
    }
    if radius<10000 {return 0;}
    let tangent=cg_cross(axis,puppy_arc+24);k=0;
    while k<24 {store64(puppy_arc+48+k,load64(tangent+k)/1000000);k=k+8;}
    let index=0;let followers=0;
    if studio {
        puppy_home=cg_number(j_get(mate,"value"));puppy_low=cg_number(j_get(mate,"minimum"));
        puppy_high=cg_number(j_get(mate,"maximum"));let fields=cad_studio_properties(project);
        while index<cg_count(fields) && load64(cg_at(fields,index))!=servo {index=index+1;}
        if index==cg_count(fields) {return 0;}
        followers=cad_studio_hinge_followers(mesh,mate);
        if puppy_harness_moves(project,followers) {followers=0;}
    }else {
        puppy_home=pc_pose_number(j_get(servo,"homeDeg"));
        let range=j_get(servo,"commandRange");puppy_low=-180000000;puppy_high=180000000;
        if j_get(range,"minDeg") {puppy_low=pc_pose_number(j_get(range,"minDeg"));}
        if j_get(range,"maxDeg") {puppy_high=pc_pose_number(j_get(range,"maxDeg"));}
        let data=j_get(cad_project_item(project),"data");let row=pc_first(j_get(data,"actuators"));
        while row && row!=servo {
            if m_eq(j_string(row,"type"),"servo") && j_get(row,"homeDeg") {index=index+1;}
            row=load64(row+24);
        }
        followers=cad_project_hinge_followers(data,mate);
    }
    puppy_angle=puppy_home;
    if cg_error || puppy_low>puppy_home || puppy_high<puppy_home {return 0;}
    if followers {
        puppy_range_bytes=16*cg_j_count(parts);
        puppy_draw_ranges=alloc(puppy_range_bytes);let k=0;let part=pc_first(parts);
        while part {
            store64(puppy_draw_ranges+k,j_integer(part,"count"));
            store64(puppy_draw_ranges+k+8,ca_find(followers,j_string(part,"id"))!=0);
            k=k+16;part=load64(part+24);
        }
    }
    puppy_selected=index;puppy_hinge=puppy_snapshot(j_string(mate,"id"));return 1;
}
fn puppy_view_press(button,x,y) {
    if puppy_button {return 0;}
    puppy_button=button;puppy_start_x=x;puppy_start_y=y;puppy_prev_x=x;puppy_prev_y=y;puppy_moved=0;
    return 1;
}
fn puppy_view_anchor_camera(button,ray,hit) {
    puppy_rim=-1;m_copy(puppy_start_basis,puppy_cam_basis,72);puppy_orbit_depth=0;
    if !ray {return 1;}
    if hit {
        m_copy(puppy_orbit_point,load64(hit),24);
        let dir=load64(ray+8);let axis=0;let k=8;
        while k<24 {if cg_abs(load64(dir+k))>cg_abs(load64(dir+axis)) {axis=k;}k=k+8;}
        puppy_orbit_depth=(load64(puppy_orbit_point+axis)-load64(load64(ray)+axis))*1000/load64(dir+axis);
    }else if button==2 {
        puppy_orbit_depth=(puppy_radius*puppy_zoom/100+2)*1000;
        m_copy(puppy_orbit_point,puppy_ray_at(ray,puppy_orbit_depth),24);
    }
    return 1;
}
fn puppy_view_down(button,x,y) {
    if !puppy_view_press(button,x,y) {return 1;}
    puppy_hover_clear();
    let source=puppy_history;if puppy_step_preview {source=puppy_step_preview;}
    let project=cad_project_open(source+24);let ray=puppy_ray(x,y);
    let hit=0;let element=0;
    if ray && !puppy_empty {
        if button==1 && m_eq(j_string(cad_project_item(project),"type"),"assembly") {
            element=puppy_mate_hit(project,ray);if element {hit=load64(element+32);}
        }else {hit=cad_project_surface_hit_in(project,load64(ray),load64(ray+8),puppy_visible_id());}
    }
    if button!=1 {return puppy_view_anchor_camera(button,ray,hit);}
    let can_toggle=!element && (cad_project_part(project) || !j_get(project,"assembly"));
    let prev_vert=puppy_selected_vertex;let prev_vx=puppy_sel_vx;let prev_vy=puppy_sel_vy;let prev_vz=puppy_sel_vz;
    let prev_edge_key=puppy_sel_edge_key;let prev_face_key=puppy_sel_face_key;
    puppy_clear_pick();
    let vert_hit=0;
    if !element && (!puppy_empty || puppy_part_sketch_available()) {vert_hit=puppy_vertex_select(x,y);}
    if vert_hit {
        if can_toggle && prev_vert && puppy_sel_vx==prev_vx && puppy_sel_vy==prev_vy && puppy_sel_vz==prev_vz {
            puppy_clear_pick();return puppy_title(project,"Selection cleared");
        }
        let coords=puppy_vertex_coords_text();
        if hit {
            let body=load64(hit+16);let parts=j_parse(puppy_catalog+24);let entry=pc_find(parts,load64(body));
            if entry {
                puppy_pick_id=puppy_snapshot(j_string(entry,"id"));
                return puppy_title(project,m_cat("Selected ",m_cat(load64(body+8),m_cat("   |   Vertex (",m_cat(coords,")")))));
            }
        }
        return puppy_title(project,m_cat("Selected Vertex (",m_cat(coords,")")));
    }
    let edge_body=0;
    if !element && ray && !puppy_empty {edge_body=puppy_edge_select(project,ray,hit);}
    if edge_body && can_toggle && prev_edge_key && puppy_sel_edge_key==prev_edge_key {
        puppy_clear_pick();return puppy_title(project,"Selection cleared");
    }
    if !element && !hit && !edge_body {return puppy_title(project,cg_error);}
    let body=edge_body;
    if !body {
        let picked=hit;if element {picked=element;}
        body=load64(picked+16);
    }
    let parts=j_parse(puppy_catalog+24);let entry=pc_find(parts,load64(body));
    if !entry {puppy_face_clear();return puppy_title(project,"Occurrence unavailable");}
    puppy_pick_id=puppy_snapshot(j_string(entry,"id"));
    if element {
        puppy_first=j_integer(entry,"first");puppy_last=puppy_first+j_integer(entry,"count");
        let frame=pm_element_frame(project,element,0);if frame && puppy_mate_mark(element,0) {puppy_mate_selection_frame=puppy_snapshot(j_encode(frame));}
    }else if !edge_body {
        if !puppy_face_select(project,ray,hit) {
            puppy_first=j_integer(entry,"first");puppy_last=puppy_first+j_integer(entry,"count");
        }else if can_toggle && prev_face_key && puppy_sel_face_key==prev_face_key {
            puppy_clear_pick();return puppy_title(project,"Selection cleared");
        }
    }
    if hit && load64(hit+16)==body {puppy_set_hinge(project,hit,parts);}
    let label=m_cat("Selected ",load64(body+8));if element {label=m_cat(label,m_cat("   |   ",load64(element+24)));}
    else if edge_body {label=m_cat(label,"   |   Edge");}
    else if puppy_face_fill_count {label=m_cat(label,"   |   Face");}
    return puppy_title(project,label);
}
fn puppy_arc_error(ray,direction,angle) {
    let c=cg_cos((angle-puppy_home)/1000);let s=cg_sin((angle-puppy_home)/1000);let k=0;
    let offset=m_take(24);
    while k<24 {
        let p=load64(puppy_arc+k)+(c*load64(puppy_arc+24+k)+s*load64(puppy_arc+48+k))/1000000;
        store64(offset+k,(p-load64(load64(ray)+k))/1000);k=k+8;
    }
    let depth=cg_dot(offset,direction);if depth<0 {return 0x7fffffffffffffff;}
    let error=0;k=0;while k<24 {
        let miss=load64(offset+k)-depth*load64(direction+k)/1000000000;
        error=error+miss*miss;k=k+8;
    }
    return error;
}
fn puppy_apply_angle(project,angle) {
    if cad_project_is_studio(project) {return cad_studio_set_property(project,puppy_selected,angle);}
    return cad_project_servo_angle(project,puppy_hinge+24,angle);
}
fn puppy_drag_hinge(x,y) {
    let ray=puppy_ray(x,y);if !ray {return 1;}
    let direction=cg_normal(load64(ray+8));let best=puppy_angle;
    let error=puppy_arc_error(ray,direction,best);let step=4000000;
    // Follow the nearest local branch of the projected circle, including
    // edge-on views. Refine below a tenth of a degree at the end.
    while step>=10000 {
        let before=best;let i=-1;
        while i<=1 {
            let trial=puppy_limit(before+i*step,puppy_low,puppy_high);
            if i && trial!=before {
                let score=puppy_arc_error(ray,direction,trial);
                if score<error {error=score;best=trial;}
            }
            i=i+2;
        }
        if best==before {step=step/4;}
    }
    if best==puppy_angle {return 1;}
    let original=cad_project_open(puppy_history+24);
    if puppy_draw_ranges {
        puppy_angle=best;
        return puppy_title(original,m_cat("Dragging hinge  ",m_cat(puppy_display_number(best)," deg")));
    }
    let edit=puppy_apply_angle(original,best);
    if !edit || !puppy_upload(edit,cad_project_mesh(edit)) {return puppy_title(original,cg_error);}
    puppy_angle=best;return puppy_title(edit,"Dragging hinge");
}
fn puppy_view_move(x,y) {
    if !puppy_moved && cg_abs(x-puppy_start_x)+cg_abs(y-puppy_start_y)<4 {return 1;}
    puppy_moved=1;let dx=x-puppy_prev_x;let dy=y-puppy_prev_y;puppy_prev_x=x;puppy_prev_y=y;
    if puppy_button==1 {if puppy_hinge {return puppy_drag_hinge(x,y);}return 1;}
    let window=load64(puppy_device);let h=gui_height(window);let w=gui_width(window);if !h || !w {return 1;}
    if puppy_button==3 {
        let rx=puppy_start_x-w/2;let ry=puppy_start_y-h/2;
        if puppy_rim<0 {
            let tangential=cg_abs(rx*dy-ry*dx);let radial=cg_abs(rx*dx+ry*dy);
            puppy_rim=(cg_abs(rx)*100>=44*w || cg_abs(ry)*100>=44*h) && 3*tangential>4*radial;
        }
        if puppy_rim {
            let end_x=x-w/2;let end_y=y-h/2;
            let delta=pc_angle_degrees(rx*end_y-ry*end_x,rx*end_x+ry*end_y)/1000;
            puppy_camera_roll_basis(puppy_cam_basis,puppy_start_basis,delta);
        }else {
            puppy_camera_rotate(dx*180000/h,dy*180000/h);
        }
        x=puppy_start_x;y=puppy_start_y;
    }
    if puppy_orbit_depth {
        let ray=puppy_ray(x,y);if !ray {return 1;}
        let point=puppy_ray_at(ray,puppy_orbit_depth);let k=0;
        while k<24 {
            let shift=load64(puppy_orbit_point+k)-load64(point+k);
            store64(puppy_pan+k,puppy_limit(load64(puppy_pan+k)+shift,-1000000000,1000000000));k=k+8;
        }
    }
    return 1;
}
fn puppy_open_context_menu(x,y,pick) {
    let project=cad_project_open(puppy_history+24);
    if puppy_selected_vertex {return puppy_vmenu_open_at(1,x,y);}
    if !cad_project_is_studio(project) {
        if pick && !puppy_empty && puppy_vertex_select(x,y) {
            let ray=puppy_ray(x,y);
            if ray {
                let hit=cad_project_surface_hit_in(project,load64(ray),load64(ray+8),puppy_visible_id());
                if hit {
                    let entry=pc_find(j_parse(puppy_catalog+24),load64(load64(hit+16)));
                    if entry {
                        if puppy_pick_id {puppy_drop(puppy_pick_id);}
                        puppy_pick_id=puppy_snapshot(j_string(entry,"id"));
                    }
                }
                cg_error=0;
            }
            return puppy_vmenu_open_at(1,x,y);
        }
        if puppy_face_tube_count {return puppy_vmenu_open_at(2,x,y);}
        if puppy_face_fill_count {return puppy_vmenu_open_at(3,x,y);}
        return 0;
    }
    let window=load64(puppy_device);let h=gui_height(window);let w=gui_width(window);
    if h<196 || w<104 {return 0;}
    let parts=j_parse(puppy_catalog+24);let entry=0;
    if pick {
        let ray=puppy_ray(x,y);
        if ray {
            let hit=cad_project_surface_hit_in(project,load64(ray),load64(ray+8),puppy_visible_id());
            if hit {entry=pc_find(parts,load64(load64(hit+16)));}
        }
    }
    if !entry && puppy_pick_id {entry=pc_find(parts,puppy_pick_id+24);}
    if !entry && puppy_definition {
        let copies=puppy_part_copies(parts);
        if cg_count(copies)==1 {entry=cg_at(copies,0);}
    }
    let assembly=j_get(project,"assembly");puppy_menu_first=2;puppy_menu_rows=1;let label="Fit view";
    if entry && assembly && j_get(entry,"part") {
        puppy_select_entry(entry);puppy_menu_first=0;puppy_menu_rows=2;label="Fix ";
        if j_get(j_get(assembly,"fixed_instances"),puppy_pick_id+24) {puppy_menu_first=1;label="Unfix ";}
        label=m_cat(label,j_string(entry,"name"));
    }
    puppy_menu_selected=0;puppy_tree_dirty=1;puppy_views_open=0;puppy_search_focus(0);
    puppy_menu_x=puppy_limit(x,0,w-96);puppy_menu_y=puppy_limit(y,76,h-24*puppy_menu_rows);
    return puppy_title(project,label);
}
fn puppy_view_end(commit) {
    let menu=commit && puppy_button==3 && !puppy_moved;
    let click=commit && puppy_button==1 && !puppy_moved && puppy_pick_id && puppy_body_count>1 && !puppy_isolated;
    puppy_button=0;
    if !click {puppy_forget_click();}
    if menu {return puppy_open_context_menu(puppy_start_x,puppy_start_y,1);}
    if !puppy_hinge {
        if click {return puppy_occurrence_click();}return 1;
    }
    let project=cad_project_open(puppy_history+24);
    if puppy_angle!=puppy_home {
        let edit=0;if commit {edit=puppy_apply_angle(project,puppy_angle);}
        if edit && puppy_upload(edit,cad_project_mesh(edit)) {puppy_push(edit);puppy_title(edit,"Hinge moved");}
        else {if !puppy_draw_ranges {puppy_upload(project,cad_project_mesh(project));}puppy_title(project,"Drag cancelled");}
    }
    if puppy_draw_ranges {syscall(11,puppy_draw_ranges,puppy_range_bytes,0,0,0,0);puppy_draw_ranges=0;}
    puppy_drop(puppy_hinge);puppy_hinge=0;
    if click {return puppy_occurrence_click();}return 1;
}
