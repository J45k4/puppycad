// The modal displays evaluated geometry in buffers separate from the model.
// Its camera changes are discarded when the modal exits.
global puppy_extrusion_view_active=0;
global puppy_extrusion_view_camera=0;
global puppy_extrusion_view_base=0;
global puppy_extrusion_view_candidate=0;
global puppy_extrusion_view_base_bounds=0;
global puppy_extrusion_view_next_bounds=0;
global puppy_extrusion_view_surface=0;
global puppy_extrusion_view_bounds=0;
global puppy_extrusion_view_base_ready=0;
global puppy_extrusion_view_adjusted=0;

fn puppy_extrusion_view_init() {
    puppy_extrusion_view_camera=m_take(96);
    puppy_extrusion_view_base_bounds=m_take(48);
    puppy_extrusion_view_next_bounds=m_take(48);
}

fn puppy_extrusion_view_upload(surface,mesh,bounds,preview) {
    let vertices=puppy_mesh_vertices(mesh,bounds,bounds+24);
    if !vertices {return 0;}
    let data=m_take(j_integer(mesh,"triangles")*48);let at=0;
    let part=pc_first(j_get(mesh,"parts"));
    while part {
        let rgb=puppy_part_rgb(part);
        if preview {rgb=0x53b7d2;}
        let index=pc_first(j_get(part,"triangles"));
        while index {
            m_copy(data+at,vertices+j_value(index)*12,12);
            puppy_vertex_color(data+at,rgb);at=at+16;index=load64(index+24);
        }
        part=load64(part+24);
    }
    return render_mesh_upload(surface,data,at/16,0x88e8);
}

fn puppy_extrusion_view_begin(project) {
    if puppy_extrusion_view_active {return 1;}
    return puppy_extrusion_view_begin_mesh(j_get(cad_project_feature_history(project),"mesh"));
}

fn puppy_extrusion_view_begin_mesh(mesh) {
    if puppy_extrusion_view_active {return 1;}
    let saved=puppy_extrusion_view_camera;
    m_copy(saved,puppy_center,24);m_copy(saved+24,puppy_pan,24);
    store64(saved+48,puppy_radius);store64(saved+56,puppy_yaw);
    store64(saved+64,puppy_pitch);store64(saved+72,puppy_zoom);
    store64(saved+80,puppy_roll);store64(saved+88,puppy_orthographic);
    puppy_extrusion_view_active=1;puppy_extrusion_view_adjusted=0;
    puppy_extrusion_view_base_ready=0;
    puppy_yaw=45000;puppy_pitch=35264;puppy_roll=0;
    if mesh {
        if !puppy_extrusion_view_base {puppy_extrusion_view_base=puppy_colored_mesh();}
        if !puppy_extrusion_view_base || !puppy_extrusion_view_upload(puppy_extrusion_view_base,mesh,puppy_extrusion_view_base_bounds,0) {
            puppy_extrusion_view_end();return 0;
        }
        puppy_extrusion_view_base_ready=1;
    }
    return puppy_extrusion_view_show(0);
}

fn puppy_extrusion_view_fit() {
    if puppy_extrusion_view_bounds {
        let bounds=puppy_extrusion_view_bounds;
        puppy_frame_bounds(bounds,bounds+24);
    }
    gui__zero(puppy_pan,24);puppy_zoom=280;puppy_extrusion_view_adjusted=0;
    return 1;
}

fn puppy_extrusion_view_show(mesh) {
    puppy_extrusion_view_surface=0;puppy_extrusion_view_bounds=0;
    if mesh {
        if !puppy_extrusion_view_candidate {puppy_extrusion_view_candidate=puppy_colored_mesh();}
        if !puppy_extrusion_view_candidate || !puppy_extrusion_view_upload(puppy_extrusion_view_candidate,mesh,puppy_extrusion_view_next_bounds,1) {return 0;}
        puppy_extrusion_view_surface=puppy_extrusion_view_candidate;
        puppy_extrusion_view_bounds=puppy_extrusion_view_next_bounds;
    }else if puppy_extrusion_view_base_ready {
        puppy_extrusion_view_surface=puppy_extrusion_view_base;
        puppy_extrusion_view_bounds=puppy_extrusion_view_base_bounds;
    }
    if !puppy_extrusion_view_adjusted {puppy_extrusion_view_fit();}
    return 1;
}

fn puppy_extrusion_view_end() {
    if !puppy_extrusion_view_active {return 1;}
    let saved=puppy_extrusion_view_camera;
    m_copy(puppy_center,saved,24);m_copy(puppy_pan,saved+24,24);
    puppy_radius=load64(saved+48);puppy_yaw=load64(saved+56);
    puppy_pitch=load64(saved+64);puppy_zoom=load64(saved+72);
    puppy_roll=load64(saved+80);puppy_orthographic=load64(saved+88);
    puppy_extrusion_view_active=0;puppy_button=0;
    return 1;
}

fn puppy_extrusion_view_pick(project,x,y) {
    if !puppy_extrusion_view_base_ready {return 0;}
    let ray=puppy_ray(x,y);if !ray {return 0;}
    let hit=cad_project_surface_hit(project,load64(ray),load64(ray+8));
    if !hit {return 0;}
    return load64(load64(hit+16));
}

fn puppy_extrusion_view_event(event) {
    if !puppy_extrusion_view_active {return 0;}
    let kind=gui_event_kind(event);let button=gui_event_button(event);
    if kind==GUI_MOUSE_DOWN && (button==2 || button==3) && gui_event_y(event)>=76 {
        let x=gui_event_x(event);let y=gui_event_y(event);
        if puppy_view_press(button,x,y) {puppy_view_anchor_camera(button,puppy_ray(x,y),0);}
        return 1;
    }
    if puppy_button {
        if kind==GUI_MOUSE_MOVE || (kind==GUI_MOUSE_UP && puppy_button==button) {
            puppy_view_move(gui_event_x(event),gui_event_y(event));
            if puppy_moved {puppy_extrusion_view_adjusted=1;}
            if kind==GUI_MOUSE_UP {puppy_button=0;}
            return 1;
        }
        if kind==GUI_FOCUS && !gui_event_code(event) {puppy_button=0;return 1;}
    }
    if kind==GUI_WHEEL && !puppy_button {
        puppy_zoom=puppy_limit(puppy_zoom-gui_event_dy(event)*15,120,800);
        puppy_extrusion_view_adjusted=1;return 1;
    }
    return 0;
}

fn puppy_extrusion_view_draw() {
    let surface=puppy_extrusion_view_surface;if !surface {return 1;}
    render_gl(puppy_device,"glUniform1i",render_uniform(puppy_material,0,"moving"),0,0,0,0,0);
    render_gl(puppy_device,"glUniform1i",render_uniform(puppy_material,0,"lineOverlay"),0,0,0,0,0);
    return render_mesh_draw(surface,puppy_material,0,0,load64(surface+32));
}

fn puppy_extrusion_view_close() {
    render_mesh_close(puppy_extrusion_view_base);
    return render_mesh_close(puppy_extrusion_view_candidate);
}
