// Project each labelled cube face onto the camera's right and up axes. Mouse
// picking inverts the same two projected edges used to draw each face.
global puppy_cube_program=0;
global puppy_cube_atlas=0;
global puppy_views_open=0;
global puppy_menu_rows=0;
global puppy_menu_x=0;
global puppy_menu_y=0;
// The context menu starts with either Fix, Unfix or Fit view; Fit view is last.
global puppy_menu_first=0;
global puppy_menu_selected=0;
global PUPPY_VIEW_ATLAS_HEIGHT=912;
fn puppy_view_name(face) {
    if face==0 {return "Front";}if face==1 {return "Right";}if face==2 {return "Top";}
    if face==3 {return "Back";}if face==4 {return "Left";}if face==5 {return "Bottom";}return "Isometric";
}
fn puppy_standard_view(face) {
    puppy_yaw=45000;puppy_pitch=35264;puppy_roll=0;puppy_views_open=0;
    if face==0 {puppy_yaw=90000;puppy_pitch=0;}
    if face==1 {puppy_yaw=0;puppy_pitch=0;}
    if face==2 {puppy_yaw=90000;puppy_pitch=90000;}
    if face==3 {puppy_yaw=-90000;puppy_pitch=0;}
    if face==4 {puppy_yaw=180000;puppy_pitch=0;}
    if face==5 {puppy_yaw=90000;puppy_pitch=-90000;}
    let project=cad_project_open(puppy_history+24);
    if cad_project_is_studio(project) {gui__zero(puppy_pan,24);puppy_zoom=280;}
    return puppy_title(project,m_cat(puppy_view_name(face)," view"));
}
fn puppy_cube_init() {
    let v="#version 450 core\nout vec2 uv;uniform vec2 center,across,down,band;void main(){int i=gl_VertexID;vec2 p=vec2(i==1||i==3||i==4?1:-1,i==2||i==4||i==5?1:-1);vec2 xy=center+p.x*across+p.y*down;gl_Position=vec4(xy.x,-xy.y,0,1);uv=vec2((p.x+1)*0.5,band.x+(p.y+1)*0.5*band.y);}";
    let f="#version 450 core\nin vec2 uv;uniform sampler2D atlas;out vec4 color;void main(){color=texture(atlas,uv);}";
    puppy_cube_program=render_program(puppy_device,v,f);
    if !puppy_cube_program {return 0;}
    let name=m_take(8);store64(name,0);render_gl(puppy_device,"glGenTextures",1,name,0,0,0,0);
    puppy_cube_atlas=render_track(puppy_device,4,gui__u32(name));if !puppy_cube_atlas {return 0;}
    // Six square face labels followed by the view chooser and normal/focused context actions.
    let font=tf_font_open(puppy_toolbar_context,"sans-serif",28,0,0);if !font {return 0;}
    let w=96;let h=PUPPY_VIEW_ATLAS_HEIGHT;let rgb=m_take(w*h*4);let i=0;
    while i<6 {
        let y=i*96;puppy_ui_rect(rgb,w,h,0,y,96,96,92,129,126);
        puppy_ui_rect(rgb,w,h,3,y+3,90,90,44+(i%3)*3,66+(i%3)*3,69+(i%3)*3);
        let label=puppy_view_name(i);let shape=tf_shape(font,label,m_len(label),0);
        let width=0;if shape {
            let j=0;while j<load64(shape) {width=width+load64(shape+64+j*48+16);j=j+1;}tf_shape_free(shape);
        }
        puppy_ui_font_text(rgb,w,h,(96-width/64)/2,y+58,label,95,font);i=i+1;
    }
    i=0;while i<14 {
        let y=576+i*24;let label="Views";if i {label=puppy_view_name(i-1);}
        if i>=8 {
            let action=(i-8)%3;label="Fit view";if action==0 {label="Fix";}if action==1 {label="Unfix";}
        }
        puppy_ui_rect(rgb,w,h,0,y,96,24,36,53,57);
        puppy_ui_rect(rgb,w,h,0,y+23,96,1,63,87,88);
        if i>=11 {puppy_ui_rect(rgb,w,h,1,y+2,3,20,143,217,184);}
        puppy_ui_text(rgb,w,h,10,y+17,label,94);
        if !i {
            let k=0;while k<4 {puppy_ui_rect(rgb,w,h,76+k,y+10+k,7-2*k,1,200,216,214);k=k+1;}
        }
        i=i+1;
    }
    render_gl(puppy_device,"glBindTexture",0x0de1,puppy_cube_atlas,0,0,0,0);puppy_ui_upload(rgb,w,h);
    render_gl(puppy_device,"glTexParameteri",0x0de1,0x2801,0x2601,0,0,0);
    render_gl(puppy_device,"glTexParameteri",0x0de1,0x2800,0x2601,0,0,0);
    return 1;
}
fn puppy_cube_basis() {
    let eye=puppy_eye(puppy_radius*puppy_zoom/100+2);let direction=cg_sub(puppy_center,eye);
    let vertical=puppy_view_up(direction);render_normalize(direction);
    let right=m_take(24);render_cross(right,direction,vertical);render_normalize(right);
    let up=m_take(24);render_cross(up,right,direction);
    return cg_v(right,up,direction);
}
fn puppy_axis_component(vector,axis) {
    if axis<0 {return -load64(vector+(-axis-1)*8);}return load64(vector+(axis-1)*8);
}
fn puppy_cube_face(basis,face) {
    let normal=-2;let across=1;let down=-3;
    if face==1 {normal=1;across=2;}
    if face==2 {normal=3;down=-2;}
    if face==3 {normal=2;across=-1;}
    if face==4 {normal=-1;across=-2;}
    if face==5 {normal=-3;down=2;}
    if puppy_axis_component(load64(basis+16),normal)>=-1 {return 0;}
    let right=load64(basis);let up=load64(basis+8);let out=m_take(48);
    store64(out,puppy_axis_component(right,normal));store64(out+8,-puppy_axis_component(up,normal));
    store64(out+16,puppy_axis_component(right,across));store64(out+24,-puppy_axis_component(up,across));
    store64(out+32,puppy_axis_component(right,down));store64(out+40,-puppy_axis_component(up,down));return out;
}
fn puppy_cube_uniform(name,x,y,scale) {
    let pair=m_take(8);render_float(pair,x,scale);render_float(pair+4,y,scale);
    let location=render_gl(puppy_device,"glGetUniformLocation",puppy_cube_program,name,0,0,0,0);
    return render_gl(puppy_device,"glUniform2fv",location,1,pair,0,0,0);
}
fn puppy_cube_rect(x,y,w,h) {
    let window=load64(puppy_device);let width=gui_width(window);let height=gui_height(window);
    let pw=gui_pixel_width(window);let ph=gui_pixel_height(window);
    return render_gl(puppy_device,"glViewport",x*pw/width,ph-(y+h)*ph/height,(x+w)*pw/width-x*pw/width,(y+h)*ph/height-y*ph/height,0,0);
}
fn puppy_cube_draw() {
    if puppy_sketch || puppy_empty {puppy_views_open=0;return 1;}
    let window=load64(puppy_device);let x=gui_width(window)-104;let h=gui_height(window);
    if x<0 || h<196 {return 1;}
    render_gl(puppy_device,"glActiveTexture",0x84c0,0,0,0,0,0);
    if !puppy_cube_program && !puppy_cube_init() {return 0;}
    render_gl(puppy_device,"glBindTexture",0x0de1,puppy_cube_atlas,0,0,0,0);
    render_gl(puppy_device,"glUseProgram",puppy_cube_program,0,0,0,0,0);
    let basis=puppy_cube_basis();puppy_cube_rect(x,80,96,96);
    let i=0;while i<6 {
        let face=puppy_cube_face(basis,i);if face {
            puppy_cube_uniform("center",load64(face),load64(face+8),1800000);
            puppy_cube_uniform("across",load64(face+16),load64(face+24),1800000);
            puppy_cube_uniform("down",load64(face+32),load64(face+40),1800000);
            puppy_cube_uniform("band",i*96,96,PUPPY_VIEW_ATLAS_HEIGHT);
            if !render_draw(puppy_device,puppy_toolbar_vao,4,0,6) {return 0;}
        }
        i=i+1;
    }
    puppy_cube_uniform("center",0,0,1);puppy_cube_uniform("across",1,0,1);puppy_cube_uniform("down",0,1,1);
    i=0;let count=1;if puppy_views_open {count=8;}
    // Keep all seven choices reachable on short windows.
    let row=puppy_limit((h-170)/8,1,24);
    while i<count {
        puppy_cube_rect(x,170+i*row,96,row);puppy_cube_uniform("band",576+i*24,24,PUPPY_VIEW_ATLAS_HEIGHT);
        if !render_draw(puppy_device,puppy_toolbar_vao,4,0,6) {return 0;}i=i+1;
    }
    if puppy_menu_rows {
        i=0;while i<puppy_menu_rows {
            let action=puppy_menu_first;if i {action=2;}
            if i==puppy_menu_selected {action=action+3;}
            puppy_cube_rect(puppy_menu_x,puppy_menu_y+i*24,96,24);
            puppy_cube_uniform("band",768+action*24,24,PUPPY_VIEW_ATLAS_HEIGHT);
            if !render_draw(puppy_device,puppy_toolbar_vao,4,0,6) {return 0;}i=i+1;
        }
    }
    return 1;
}
fn puppy_menu_activate(row) {
    puppy_menu_rows=0;
    if row || puppy_menu_first==2 {return puppy_fit();}
    return puppy_command(19,0);
}
fn puppy_context_event(event) {
    let kind=gui_event_kind(event);let key=gui_event_code(event);
    if !puppy_menu_rows {
        if kind!=GUI_KEY_DOWN || puppy_search_active || puppy_sketch || puppy_empty || puppy_button {return 0;}
        if key!=101 && key!=118 && !(key==67 && (gui_event_modifiers(event)&3)) {return 0;}
        let window=load64(puppy_device);
        return puppy_open_context_menu(gui_width(window)/2,gui_height(window)/2,0);
    }
    if kind==GUI_MOUSE_DOWN || kind==GUI_MOUSE_MOVE {
        let x=gui_event_x(event)-puppy_menu_x;let y=gui_event_y(event)-puppy_menu_y;
        let inside=x>=0 && x<96 && y>=0 && y<24*puppy_menu_rows;
        if kind==GUI_MOUSE_MOVE {
            if inside {puppy_menu_selected=y/24;}return 1;
        }
        if inside && gui_event_button(event)==1 {return puppy_menu_activate(y/24);}
        puppy_menu_rows=0;return 0;
    }
    if kind==GUI_KEY_DOWN {
        if key==40 || key==88 || key==44 {return puppy_menu_activate(puppy_menu_selected);}
        if key==GUI_KEY_DOWN_ARROW || key==GUI_KEY_UP_ARROW {puppy_menu_selected=(puppy_menu_selected+1)%puppy_menu_rows;}
        else if key==74 {puppy_menu_selected=0;}
        else if key==77 {puppy_menu_selected=puppy_menu_rows-1;}
        else if key==GUI_KEY_ESCAPE || key==43 {
            puppy_menu_rows=0;puppy_title(cad_project_open(puppy_history+24),"Menu dismissed");
        }
        return 1;
    }
    if kind==GUI_RESIZE || kind==GUI_FOCUS && !gui_event_code(event) {puppy_menu_rows=0;return 1;}
    if kind==GUI_WHEEL {return 1;}
    return 0;
}
fn puppy_cube_click(x,y) {
    if puppy_sketch || puppy_empty {return 0;}
    let window=load64(puppy_device);let h=gui_height(window);x=x-gui_width(window)+104;
    if h<196 || x<0 || x>=96 || y<80 {puppy_views_open=0;return 0;}
    let row=puppy_limit((h-170)/8,1,24);
    if y>=170 && y<170+row {puppy_views_open=!puppy_views_open;return 1;}
    if puppy_views_open && y>=170+row && y<170+8*row {return puppy_standard_view((y-170)/row-1);}
    puppy_views_open=0;if y>=170 {return 0;}
    let px=(x-48)*1800000/48;let py=(y-128)*1800000/48;
    let basis=puppy_cube_basis();let i=0;
    while i<6 {
        let face=puppy_cube_face(basis,i);if face {
            let dx=px-load64(face);let dy=py-load64(face+8);
            let a=load64(face+16);let b=load64(face+24);let c=load64(face+32);let d=load64(face+40);
            let determinant=cg_abs(a*d-b*c);
            if determinant && cg_abs(dx*d-dy*c)<=determinant && cg_abs(a*dy-b*dx)<=determinant {
                return puppy_standard_view(i);
            }
        }
        i=i+1;
    }
    return 0;
}
