import "render";
import "toolbar.flex";
import "sketch-view.flex";
import "draft.flex";
import "viewport-input.flex";
import "objects.flex";
import "harness.flex";
import "view-cube.flex";
import "drawing-view.flex";
import "part-sketches.flex";
import "extrusion-preview.flex";
import "face-selection.flex";
import "viewport-menu.flex";

global puppy_device=0;
global puppy_empty=0;
global puppy_surface=0;
global puppy_material=0;
global puppy_center=0;
global puppy_radius=1;
global puppy_cam_basis=0;
global puppy_start_basis=0;
global puppy_zoom=280;
global puppy_orthographic=0;
global puppy_solved_assembly=0;
global puppy_mesh_bounds=0;

// Positions use three floats, followed by a normalized RGB and padding byte.
fn puppy_colored_mesh() {
    let attrs=m_take(96);gui__zero(attrs,96);store64(attrs+8,3);store64(attrs+16,0x1406);
    store64(attrs+48,1);store64(attrs+56,3);store64(attrs+64,0x1401);
    store64(attrs+72,1);store64(attrs+80,12);
    return render_mesh(puppy_device,16,attrs,2);
}
// Shaded solid surfaces add a smooth per-corner normal after position and color.
fn puppy_shaded_mesh() {
    let attrs=m_take(144);gui__zero(attrs,144);store64(attrs+8,3);store64(attrs+16,0x1406);
    store64(attrs+48,1);store64(attrs+56,3);store64(attrs+64,0x1401);
    store64(attrs+72,1);store64(attrs+80,12);
    store64(attrs+96,2);store64(attrs+104,3);store64(attrs+112,0x1406);
    store64(attrs+128,16);
    return render_mesh(puppy_device,28,attrs,3);
}
fn puppy_vertex_color(vertex,rgb) {
    store8(vertex+12,rgb>>16);store8(vertex+13,rgb>>8);store8(vertex+14,rgb);store8(vertex+15,255);
}
fn puppy_part_rgb(part) {
    let value=j_string(part,"color");if !cg_color(value) {return 0x66ad94;}
    let rgb=0;let i=1;while i<7 {rgb=rgb*16+j_hex(load8(value+i));i=i+1;}return rgb;
}

fn puppy_view_open() {
    puppy_device=render_open("PuppyCAD",960,640,GUI_RESIZABLE|GUI_HIGH_DPI);
    if !puppy_device {return 0;}
    let vertex="#version 450 core\nlayout(location=0) in vec3 position;layout(location=1) in vec3 vertexColor;layout(location=2) in vec3 vertexNormal;flat out vec3 ink; uniform mat4 camera; uniform vec2 ortho; out vec3 world;out vec3 vNormal;flat out int picked;uniform ivec2 selection;uniform vec3 cameraPan;uniform int moving,lineOverlay;uniform vec3 axis,pivot;uniform vec2 cs; void main(){ink=vertexColor;picked=int(gl_VertexID>=selection.x && gl_VertexID<selection.y);world=position;vNormal=vertexNormal;if(moving!=0){vec3 r=position-pivot;world=pivot+cs.x*r+cs.y*cross(axis,r)+(1.0-cs.x)*dot(axis,r)*axis;vNormal=cs.x*vNormal+cs.y*cross(axis,vNormal)+(1.0-cs.x)*dot(axis,vNormal)*axis;}gl_Position=camera*vec4(world-cameraPan,1.0);if(ortho.x>0.0){gl_Position.z=ortho.x*(2.0*(gl_Position.w-0.001)/(ortho.y-0.001)-1.0);gl_Position.w=ortho.x;}if(lineOverlay==2){gl_Position.z-=0.00025*gl_Position.w;}if(lineOverlay==3){gl_Position.z-=0.00002*gl_Position.w;}gl_PointSize=5.0;}";
    let fragment="#version 450 core\nin vec3 world;in vec3 vNormal;flat in int picked;flat in vec3 ink;uniform int lineOverlay;out vec4 color; void main(){if(lineOverlay==1 || lineOverlay==2){color=vec4(ink,1.0);return;}vec3 n=dot(vNormal,vNormal)>0.01?normalize(vNormal):normalize(cross(dFdx(world),dFdy(world)));float light=max(0.0,dot(n,normalize(vec3(-0.4,-0.6,1.0))));color=vec4((picked==1?vec3(0.98,0.69,0.28):ink)*(0.54+light*0.46),1.0);}";
    puppy_material=render_material(puppy_device,vertex,fragment);
    if !puppy_material {return 0;}
    puppy_surface=puppy_shaded_mesh();
    puppy_center=cg_v(0,0,0);puppy_pan=cg_v(0,0,0);puppy_arc=m_take(96);puppy_orbit_point=m_take(24);
    puppy_cam_basis=m_take(72);puppy_start_basis=m_take(72);puppy_camera_set_angles(42000,26000,0);
    puppy_sel_face_normal=m_take(24);
    puppy_mesh_bounds=m_take(48);puppy_part_sketch_bounds=m_take(48);
    puppy_extrusion_view_init();
    return puppy_surface && puppy_toolbar_open() && puppy_sketch_open();
}
fn puppy_frame_bounds(low,high) {
    if puppy_hinge {return 1;}
    let size=0;let k=0;
    while k<24 {
        let a=load64(low+k);let b=load64(high+k);
        store64(puppy_center+k,(a+b)/2000000);
        if b-a>size {size=b-a;}
        k=k+8;
    }
    puppy_radius=(size+999999)/1000000;
    if !puppy_radius {puppy_radius=1;}
    return 1;
}
fn puppy_mesh_vertices(mesh,low,high) {
    let points=pc_first(j_get(mesh,"vertices"));
    let count=cg_j_count(j_get(mesh,"vertices"));
    if !count || j_get(mesh,"error") {return 0;}
    let vertices=m_take(count*12);let index=0;
    while points {
        let component=pc_first(points);let k=0;
        while k<3 {
            let n=cg_number(component);
            render_float(vertices+index*12+k*4,n,1000000000);
            if !index || n<load64(low+k*8) {store64(low+k*8,n);}
            if !index || n>load64(high+k*8) {store64(high+k*8,n);}
            component=load64(component+24);k=k+1;
        }
        points=load64(points+24);index=index+1;
    }
    return vertices;
}
fn puppy_upload(project,mesh) {
    let low=cg_v(0,0,0);let high=cg_v(0,0,0);
    let vertices=puppy_mesh_vertices(mesh,low,high);if !vertices {return 0;}
    let parts=j_get(mesh,"parts");puppy_body_count=cg_j_count(parts);
    let catalog=j_array();let instances=j_get(j_get(mesh,"assembly"),"instances");let source_field="part";
    if !cad_project_is_studio(project) {instances=j_get(j_get(cad_project_item(project),"data"),"instances");source_field="partId";}
    let isolated=0;
    if puppy_isolated {
        isolated=pc_find(parts,puppy_isolated+24);
        if isolated {
            let bounds=pc_first(j_get(isolated,"bounds"));
            low=cg_vector(bounds,0);high=cg_vector(load64(bounds+24),0);
        }else {puppy_end_isolation();}
    }
    if !puppy_upload_harness(mesh,low,high) {return 0;}
    m_copy(puppy_mesh_bounds,low,24);m_copy(puppy_mesh_bounds+24,high,24);
    puppy_frame_bounds(low,high);
    puppy_hover_cache_mesh(mesh,isolated);
    let triangles=j_integer(mesh,"triangles");
    let data=m_take(triangles*84);let at=0;let corner=0;
    puppy_visible_first=0;puppy_visible_count=triangles*3;
    puppy_first=0;puppy_last=0;puppy_face_clear();let part=pc_first(parts);
    while part {
        let rgb=puppy_part_rgb(part);let start=corner;let id=pc_first(j_get(part,"triangles"));
        while id {
            m_copy(data+at,vertices+j_value(id)*12,12);
            puppy_vertex_color(data+at,rgb);
            if puppy_hover_tri_norms && corner<puppy_hover_raw_tri_count*3 {
                m_copy(data+at+16,puppy_hover_tri_norms+corner*12,12);
            }else {gui__zero(data+at+16,12);}
            at=at+28;corner=corner+1;id=load64(id+24);
        }
        let entry=j_object();cg_j_text(entry,"id",j_string(part,"id"));cg_j_text(entry,"name",j_string(part,"name"));
        let occurrence=pc_find(instances,j_string(part,"id"));
        if j_get(occurrence,source_field) {cg_j_text(entry,"part",j_string(occurrence,source_field));}
        cg_j_int(entry,"first",start);cg_j_int(entry,"count",corner-start);
        j_add(entry,"bounds",j_parse(j_encode(j_get(part,"bounds"))));j_add(catalog,0,entry);
        if part==isolated {puppy_visible_first=start;puppy_visible_count=corner-start;}
        if puppy_pick_id && m_eq(puppy_pick_id+24,j_string(part,"id")) {puppy_first=start;puppy_last=corner;}
        part=load64(part+24);
    }
    if !render_mesh_upload(puppy_surface,data,corner,0x88e4) {return 0;}
    if puppy_solved_assembly {puppy_drop(puppy_solved_assembly);puppy_solved_assembly=0;}
    let assembly=j_get(mesh,"assembly");
    if assembly {puppy_solved_assembly=puppy_snapshot(j_encode(assembly));}
    if puppy_catalog {puppy_drop(puppy_catalog);}
    puppy_catalog=puppy_snapshot(j_encode(catalog));puppy_tree_dirty=1;return 1;
}
fn puppy_camera_orthonormalize(basis) {
    let right=basis;let up=basis+24;let forward=basis+48;
    render_normalize(forward);
    render_cross(right,forward,up);render_normalize(right);
    render_cross(up,right,forward);render_normalize(up);
    return 1;
}
fn puppy_camera_roll_basis(dst,src,angle) {
    let c=cg_cos(angle);let s=cg_sin(angle);let k=0;
    while k<24 {
        let r=load64(src+k);let u=load64(src+24+k);let f=load64(src+48+k);
        store64(dst+k,(c*r-s*u)/1000000);
        store64(dst+24+k,(c*u+s*r)/1000000);
        store64(dst+48+k,f);
        k=k+8;
    }
    return puppy_camera_orthonormalize(dst);
}
fn puppy_camera_set_angles(yaw,pitch,roll) {
    let basis=puppy_cam_basis;let right=basis;let up=basis+24;let forward=basis+48;
    let cp=cg_cos(pitch);let sp=cg_sin(pitch);let cy=cg_cos(yaw);let sy=cg_sin(yaw);
    store64(forward,-cp*cy/1000000);store64(forward+8,cp*sy/1000000);store64(forward+16,-sp);
    render_normalize(forward);
    let vertical=cg_v(0,0,1000000);
    if !load64(forward) && !load64(forward+8) {
        vertical=cg_v(0,1000000,0);if load64(forward+16)>0 {store64(vertical+8,-1000000);}
    }
    render_cross(right,forward,vertical);render_normalize(right);
    render_cross(up,right,forward);render_normalize(up);
    puppy_hover_clear();
    if roll {return puppy_camera_roll_basis(basis,basis,roll);}
    return 1;
}
fn puppy_camera_align_normal(normal) {
    let basis=puppy_cam_basis;let right=basis;let up=basis+24;let forward=basis+48;
    store64(forward,-load64(normal)/1000);
    store64(forward+8,-load64(normal+8)/1000);
    store64(forward+16,-load64(normal+16)/1000);
    render_normalize(forward);
    let vertical=cg_v(0,0,1000000);
    if cg_abs(load64(forward+16))>990000 {
        vertical=cg_v(0,1000000,0);if load64(forward+16)>0 {store64(vertical+8,-1000000);}
    }
    render_cross(right,forward,vertical);render_normalize(right);
    render_cross(up,right,forward);render_normalize(up);
    puppy_hover_clear();return 1;
}
fn puppy_camera_rotate(yaw_delta,pitch_delta) {
    if !yaw_delta && !pitch_delta {return 1;}
    puppy_hover_clear();
    let basis=puppy_cam_basis;let right=basis;let up=basis+24;let forward=basis+48;
    if yaw_delta {
        let c=cg_cos(yaw_delta);let s=cg_sin(yaw_delta);let k=0;
        while k<24 {
            let f=load64(forward+k);let r=load64(right+k);
            store64(forward+k,(c*f+s*r)/1000000);
            store64(right+k,(c*r-s*f)/1000000);
            k=k+8;
        }
    }
    if pitch_delta {
        let c=cg_cos(pitch_delta);let s=cg_sin(pitch_delta);let k=0;
        while k<24 {
            let f=load64(forward+k);let u=load64(up+k);
            store64(forward+k,(c*f-s*u)/1000000);
            store64(up+k,(c*u+s*f)/1000000);
            k=k+8;
        }
    }
    return puppy_camera_orthonormalize(basis);
}
fn puppy_eye(distance) {
    let target=puppy_center;let forward=puppy_cam_basis+48;
    return cg_v(load64(target)-distance*load64(forward)/1000000,
                load64(target+8)-distance*load64(forward+8)/1000000,
                load64(target+16)-distance*load64(forward+16)/1000000);
}
fn puppy_ray(x,y) {
    let window=load64(puppy_device);let w=gui_width(window);let h=gui_height(window);
    let distance=puppy_radius*puppy_zoom/100+2;let eye=puppy_eye(distance);let forward=cg_sub(puppy_center,eye);
    let right=m_take(24);let up=m_take(24);
    if !render_normalize(forward) {return cg_fail("Invalid camera");}
    render_cross(right,forward,puppy_view_up(forward));if !render_normalize(right) {return cg_fail("Invalid camera");}
    render_cross(up,right,forward);
    // Invert the three rows used by render_camera, including its integer-mm
    // view translation. Both mouse coordinates and window size are logical pixels.
    let u=cg_cross(up,forward);let v=cg_cross(forward,right);let f=cg_cross(right,up);
    let determinant=cg_dot(right,u);if !determinant || !h {return cg_fail("Invalid camera");}
    let k=0;let origin=m_take(24);let direction=m_take(24);
    // Projected translations truncate after summing the three components.
    let scaled=cg_v(load64(eye)*1000,load64(eye+8)*1000,load64(eye+16)*1000);
    let ox=cg_dot(scaled,right);let oy=cg_dot(scaled,up);let oz=cg_dot(scaled,forward);
    let sx=(2*x+1-w)*1000000*1000/(1900*h);let sy=(h-2*y-1)*1000000*1000/(1900*h);
    while k<24 {
        store64(origin+k,load64(puppy_pan+k)+(ox*load64(u+k)+oy*load64(v+k)+oz*load64(f+k))/(determinant/1000));
        store64(direction+k,(load64(u+k)/1000000*sx+load64(v+k)/1000000*sy+load64(f+k))/(determinant/1000));
        if puppy_orthographic {
            // Parallel rays start at the pixel's offset on the eye plane.
            let axial=load64(f+k)/(determinant/1000);
            store64(origin+k,load64(origin+k)+(load64(direction+k)-axial)*distance);
            store64(direction+k,axial);
        }
        k=k+8;
    }
    return cg_v(origin,direction,0);
}
fn puppy_face_ray(project,x,y) {
    let ray=puppy_ray(x,y);if !ray {return 0;}
    return cad_project_pick_face_in(project,load64(ray),load64(ray+8),puppy_visible_id());
}
fn puppy_bind_camera(camera,distance,first,last) {
    if !render_matrix(puppy_material,0,"camera",camera) {return 0;}
    let ortho=m_take(8);render_float(ortho,distance*puppy_orthographic,1000);
    render_float(ortho+4,distance+puppy_radius*3+10,1000);
    render_gl(puppy_device,"glUniform2fv",render_uniform(puppy_material,0,"ortho"),1,ortho,0,0,0);
    let shift=m_take(12);let k=0;
    while k<3 {render_float(shift+k*4,load64(puppy_pan+k*8),1000000000);k=k+1;}
    render_gl(puppy_device,"glUniform3fv",render_uniform(puppy_material,0,"cameraPan"),1,shift,0,0,0);
    render_gl(puppy_device,"glUniform2i",render_uniform(puppy_material,0,"selection"),first,last,0,0,0);
    return 1;
}
fn puppy_draw() {
    let window=load64(puppy_device);
    let w=gui_pixel_width(window);let h=gui_pixel_height(window);
    if !w || !h {return 1;}
    if puppy_reader_source {return puppy_reader_draw_source(w,h);}
    if puppy_sheet_scene {
        if !render_begin(puppy_device,0,w,h) {return 0;}
        return puppy_toolbar_draw(w,h) && puppy_sheet_draw(w,h) && puppy_objects_draw(w,h) && gui_gl_swap(window);
    }
    if puppy_part_sketch_dirty && !puppy_sketch && !puppy_reader_report && !puppy_step_input {
        if !puppy_part_sketch_upload(cad_project_open(puppy_history+24)) {return 0;}
    }
    let distance=puppy_radius*puppy_zoom/100+2;
    let eye=puppy_eye(distance);
    let camera=m_take(64);
    if !render_camera(camera,eye,puppy_center,puppy_view_up(cg_sub(puppy_center,eye)),w,h,1900000,1,distance+puppy_radius*3+10) {return 0;}
    if !render_begin(puppy_device,0,w,h) {return 0;}
    let clear=m_take(16);
    render_float(clear,20,255);render_float(clear+4,29,255);render_float(clear+8,33,255);render_float(clear+12,1,1);
    render_gl(puppy_device,"glClearBufferfv",0x1800,0,clear,0,0,0);
    if puppy_extrusion_view_active {
        return puppy_bind_camera(camera,distance,0,0) && puppy_extrusion_view_draw()
            && puppy_toolbar_draw(w,h) && gui_gl_swap(window);
    }
    let drawn=0;
    if puppy_empty && !puppy_part_sketch_available() {drawn=1;}
    else if puppy_draft {drawn=puppy_draw_draft();}
    else if puppy_sketch {drawn=puppy_draw_sketch(w,h);}
    else {
        if !puppy_bind_camera(camera,distance,puppy_first,puppy_last) {return 0;}
        if puppy_empty {drawn=puppy_draw_face_selection();}
        else if puppy_edge_kind && puppy_edge_preview_count {drawn=puppy_edge_draw_preview() && puppy_draw_face_selection();}
        else {drawn=puppy_draw_occurrences() && puppy_draw_face_selection() && puppy_draw_harness();}
        if drawn {drawn=puppy_part_sketch_draw();}
        if drawn && puppy_mating {
            if puppy_mate_first {drawn=puppy_draw_lines(puppy_first_marker,2);}
            if drawn && puppy_mate_second {drawn=puppy_draw_lines(puppy_second_marker,2);}
        }
        if drawn && puppy_edge_kind {drawn=puppy_draw_lines(puppy_edge_mesh,2);}
    }
    if puppy_reader_report {return drawn && puppy_toolbar_draw(w,h) && gui_gl_swap(window);}
    if puppy_step_input {return drawn && puppy_toolbar_draw(w,h) && puppy_cube_draw() && gui_gl_swap(window);}
    return drawn && puppy_toolbar_draw(w,h) && puppy_objects_draw(w,h) && puppy_cube_draw() && puppy_vmenu_draw(w,h) && gui_gl_swap(window);
}
fn puppy_view_close() {
    puppy_history_close();puppy_history_diagnostic(0);puppy_vmenu_cleanup();
    if puppy_solved_assembly {puppy_drop(puppy_solved_assembly);puppy_solved_assembly=0;}
    if puppy_catalog {puppy_drop(puppy_catalog);puppy_catalog=0;}
    if puppy_folded_paths {puppy_drop(puppy_folded_paths);puppy_folded_paths=0;}
    puppy_search_focus(0);if puppy_search {puppy_drop(puppy_search);puppy_search=0;}
    if puppy_toolbar_context {tf_context_free(puppy_toolbar_context);}
    render_mesh_close(puppy_first_marker);render_mesh_close(puppy_second_marker);
    if puppy_edge_kind {puppy_edge_close();}render_mesh_close(puppy_edge_mesh);render_mesh_close(puppy_edge_preview_mesh);
    render_mesh_close(puppy_part_sketch_mesh);puppy_face_close();
    puppy_extrusion_view_close();
    render_mesh_close(puppy_harness_mesh);render_mesh_close(puppy_sketch_mesh);render_mesh_close(puppy_surface);render_material_close(puppy_material);
    render_close(puppy_device);gui_quit();return 0;
}
