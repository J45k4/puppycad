// Dynamic right-click context menu for selected 3D vertices, edges, faces, and 2D sketch vertices.
global puppy_vmenu_open=0;
global puppy_vmenu_x=0;
global puppy_vmenu_y=0;
global puppy_vmenu_choice=0;
global puppy_vmenu_tex=0;
global PUPPY_VMENU_WIDTH=196;
global PUPPY_VMENU_ROW=26;
global puppy_draft_notice=0;
global puppy_vmenu_profile=0;

fn puppy_draft_notice_clear() {
    if !puppy_draft_notice {return 0;}
    puppy_drop(puppy_draft_notice);puppy_draft_notice=0;return 1;
}
fn puppy_draft_notice_set(message) {
    puppy_draft_notice_clear();
    if message && m_len(message) {puppy_draft_notice=puppy_snapshot(message);}
    return 1;
}
fn puppy_vmenu_close() {
    if !puppy_vmenu_open {return 0;}
    puppy_vmenu_open=0;puppy_vmenu_choice=0;return 1;
}
fn puppy_vmenu_cleanup() {
    puppy_vmenu_close();puppy_draft_notice_clear();
    puppy_vmenu_tex=0;return 1;
}
fn puppy_vmenu_entries(project) {
    let rows=cg_array();
    if puppy_vmenu_open==1 {
        cg_push(rows,cg_v("Center view here",10,0));
        cg_push(rows,cg_v("Copy coordinates",11,0));
        if pc_nodes(cad_project_part(project)) {cg_push(rows,cg_v("New sketch...",12,0));}
        if !cad_project_is_studio(project) && m_eq(j_string(cad_project_item(project),"type"),"assembly") && !puppy_empty {
            cg_push(rows,cg_v("Mate",13,0));
        }
        if cad_project_is_studio(project) && puppy_pick_id && j_get(project,"assembly") {
            let pins=j_get(j_get(project,"assembly"),"fixed_instances");
            let label="Fix occurrence";if j_get(pins,puppy_pick_id+24) {label="Unfix occurrence";}
            cg_push(rows,cg_v(label,14,0));
        }
        cg_push(rows,cg_v("Fit view",15,0));
        cg_push(rows,cg_v("Clear selection",16,0));
        return rows;
    }
    if puppy_vmenu_open==2 {
        if puppy_selected_edge {
            cg_push(rows,cg_v("Fillet edge",20,0));
            cg_push(rows,cg_v("Chamfer edge",21,0));
        }
        cg_push(rows,cg_v("Fit view",15,0));
        cg_push(rows,cg_v("Clear selection",16,0));
        return rows;
    }
    if puppy_vmenu_open==3 {
        if puppy_selected_face {cg_push(rows,cg_v("New sketch on face",30,0));}
        if puppy_sel_face_normal_valid {cg_push(rows,cg_v("Normal to face",31,0));}
        cg_push(rows,cg_v("Fit view",15,0));
        cg_push(rows,cg_v("Clear selection",16,0));
        return rows;
    }
    if puppy_vmenu_open==4 {
        cg_push(rows,cg_v("Line from vertex",40,0));
        cg_push(rows,cg_v("Circle at vertex",41,0));
        cg_push(rows,cg_v("Rectangle from vertex",42,0));
        if puppy_line_button(project) {cg_push(rows,cg_v("Dimensions & constraints",43,0));}
        cg_push(rows,cg_v("Center view here",44,0));
        cg_push(rows,cg_v("Copy coordinates",45,0));
        cg_push(rows,cg_v("Clear selection",46,0));
        return rows;
    }
    if puppy_vmenu_open==5 && puppy_vmenu_profile>0 {
        let label=m_int(puppy_vmenu_profile);
        cg_push(rows,cg_v(m_cat("Extrude profile ",label),50,0));
        cg_push(rows,cg_v(m_cat("Select profile ",label),51,0));
        cg_push(rows,cg_v("Select regions",52,0));
        cg_push(rows,cg_v("Fit view",53,0));
        return rows;
    }
    return rows;
}
fn puppy_vmenu_open_at(kind,x,y) {
    let project=cad_project_open(puppy_history+24);
    puppy_vmenu_open=kind;
    let rows=puppy_vmenu_entries(project);let count=cg_count(rows);
    if !count {puppy_vmenu_open=0;return 0;}
    let window=load64(puppy_device);let w=gui_width(window);let h=gui_height(window);
    let mw=puppy_limit(PUPPY_VMENU_WIDTH,1,w-8);
    let mh=count*PUPPY_VMENU_ROW+2;
    puppy_vmenu_x=puppy_limit(x,4,w-mw-4);
    puppy_vmenu_y=puppy_limit(y,76,h-mh-4);
    puppy_vmenu_choice=0;
    puppy_menu_rows=0;puppy_views_open=0;puppy_design_menu_close();puppy_search_focus(0);
    return puppy_title(project,load64(cg_at(rows,0)));
}
fn puppy_vmenu_activate(choice) {
    let project=cad_project_open(puppy_history+24);
    let rows=puppy_vmenu_entries(project);
    let entry=cg_at(rows,choice);
    puppy_vmenu_close();
    if !entry {return 1;}
    let action=load64(entry+8);
    if action==10 {
        store64(puppy_pan,puppy_sel_vx-load64(puppy_center)*1000000);
        store64(puppy_pan+8,puppy_sel_vy-load64(puppy_center+8)*1000000);
        store64(puppy_pan+16,puppy_sel_vz-load64(puppy_center+16)*1000000);
        puppy_hover_clear();
        return puppy_title(project,"Centered on vertex");
    }
    if action==11 {
        let coords=puppy_vertex_coords_text();
        gui_clipboard_copy(coords);
        return puppy_title(project,m_cat("Copied ",coords));
    }
    if action==12 {return puppy_start_plane_chooser(project);}
    if action==13 {return puppy_command(13,0);}
    if action==14 {return puppy_command(19,0);}
    if action==15 {return puppy_fit();}
    if action==16 {puppy_clear_pick();return puppy_title(project,"Selection cleared");}
    if action==20 {return puppy_edge_begin(project,1);}
    if action==21 {return puppy_edge_begin(project,0);}
    if action==30 && puppy_selected_face {return puppy_open_face_sketch(project,j_parse(puppy_selected_face+24));}
    if action==31 && puppy_sel_face_normal_valid {
        puppy_camera_align_normal(puppy_sel_face_normal);
        return puppy_title(project,"Normal to face");
    }
    if action==40 || action==41 || action==42 {
        puppy_curves_page=0;puppy_tool=2;
        if action==41 {puppy_tool=1;}if action==42 {puppy_tool=0;}
        puppy_anchor=1;
        puppy_ax=puppy_sketch_sel_x;puppy_ay=puppy_sketch_sel_y;
        puppy_mx=puppy_sketch_sel_x;puppy_my=puppy_sketch_sel_y;
        return puppy_title(project,puppy_draft_hint());
    }
    if action==43 {return puppy_command(65,0);}
    if action==44 {
        puppy_draft_cx=puppy_sketch_sel_x;puppy_draft_cy=puppy_sketch_sel_y;
        return puppy_title(project,"Centered on vertex");
    }
    if action==45 {
        let coords=m_cat(puppy_display_number(puppy_sketch_sel_x),m_cat(", ",m_cat(puppy_display_number(puppy_sketch_sel_y)," mm")));
        gui_clipboard_copy(coords);
        return puppy_title(project,m_cat("Copied ",coords));
    }
    if action==46 {
        puppy_sketch_sel_clear();
        return puppy_title(project,"Selection cleared");
    }
    if action==50 && puppy_vmenu_profile>0 {
        puppy_profile_index=puppy_vmenu_profile;puppy_profile_selected=1;
        return puppy_extrusion_begin(project,0);
    }
    if action==51 && puppy_vmenu_profile>0 {
        puppy_profile_index=puppy_vmenu_profile;puppy_profile_selected=1;
        return puppy_title(project,m_cat("Profile ",m_int(puppy_profile_index)));
    }
    if action==52 {
        puppy_curves_page=0;puppy_anchor=0;puppy_tool=-10;
        return puppy_title(project,puppy_draft_hint());
    }
    if action==53 {
        if !puppy_fit_sketch(project,0) {return puppy_title(project,cg_error);}
        return puppy_title(project,"Sketch fitted");
    }
    return 1;
}
fn puppy_vmenu_event(event) {
    let kind=gui_event_kind(event);let key=gui_event_code(event);
    if !puppy_vmenu_open {
        if kind==GUI_KEY_DOWN && !puppy_search_active && !puppy_button
            && (key==101 || key==118 || (key==67 && (gui_event_modifiers(event)&3))) {
            let window=load64(puppy_device);
            let mx=gui_width(window)/2;let my=gui_height(window)/2;
            if puppy_draft && puppy_sketch_sel_vertex {return puppy_vmenu_open_at(4,mx,my);}
            if !puppy_sketch && !puppy_empty {
                if puppy_selected_vertex {return puppy_vmenu_open_at(1,mx,my);}
                let project=cad_project_open(puppy_history+24);
                if !cad_project_is_studio(project) {
                    if puppy_face_tube_count {return puppy_vmenu_open_at(2,mx,my);}
                    if puppy_face_fill_count {return puppy_vmenu_open_at(3,mx,my);}
                }
            }
        }
        return 0;
    }
    let project=cad_project_open(puppy_history+24);
    let rows=puppy_vmenu_entries(project);let count=cg_count(rows);
    if !count {puppy_vmenu_close();return 0;}
    let window=load64(puppy_device);let mw=puppy_limit(PUPPY_VMENU_WIDTH,1,gui_width(window)-8);
    let mh=count*PUPPY_VMENU_ROW+2;
    if kind==GUI_MOUSE_MOVE || kind==GUI_MOUSE_DOWN {
        let dx=gui_event_x(event)-puppy_vmenu_x;let dy=gui_event_y(event)-puppy_vmenu_y-1;
        let inside=dx>=0 && dx<mw && dy>=0 && dy<count*PUPPY_VMENU_ROW;
        if kind==GUI_MOUSE_MOVE {
            if inside {puppy_vmenu_choice=dy/PUPPY_VMENU_ROW;}
            return 1;
        }
        if inside && gui_event_button(event)==1 {return puppy_vmenu_activate(dy/PUPPY_VMENU_ROW);}
        puppy_vmenu_close();
        return 0;
    }
    if kind==GUI_KEY_DOWN {
        if key==40 || key==88 || key==44 {return puppy_vmenu_activate(puppy_vmenu_choice);}
        if key==GUI_KEY_UP_ARROW || key==GUI_KEY_DOWN_ARROW {
            let step=1;if key==GUI_KEY_UP_ARROW {step=count-1;}
            puppy_vmenu_choice=(puppy_vmenu_choice+step)%count;
            return puppy_title(project,load64(cg_at(rows,puppy_vmenu_choice)));
        }
        if key==74 {puppy_vmenu_choice=0;return 1;}
        if key==77 {puppy_vmenu_choice=count-1;return 1;}
        if key==GUI_KEY_ESCAPE || key==43 {
            puppy_vmenu_close();
            return puppy_title(project,"Menu dismissed");
        }
        return 1;
    }
    if kind==GUI_WHEEL || kind==GUI_RESIZE || (kind==GUI_FOCUS && !gui_event_code(event)) {
        puppy_vmenu_close();return 1;
    }
    return 0;
}
fn puppy_vmenu_draw(pw,ph) {
    if !puppy_vmenu_open {
        let window=load64(puppy_device);let w=gui_width(window);let h=gui_height(window);
        if puppy_draft_notice && !puppy_reader_report {
            let left=12;if puppy_design_open && puppy_design_available() {left=puppy_design_width()+12;}
            let max_w=w-left-12;if max_w<120 {return 1;}
            let text=puppy_draft_notice+24;
            let nw=puppy_limit(puppy_toolbar_text_width(text)+28,160,max_w);
            let nh=28;let nx=left+(max_w-nw)/2;let ny=86;
            let rgb=m_take(nw*nh*4);
            puppy_ui_rect(rgb,nw,nh,0,0,nw,nh,255,166,0);
            puppy_ui_rect(rgb,nw,nh,1,1,nw-2,nh-2,54,42,26);
            puppy_ui_text(rgb,nw,nh,14,19,text,nw-12);
            if !puppy_vmenu_tex {
                let p=m_take(8);store64(p,0);render_gl(puppy_device,"glGenTextures",1,p,0,0,0,0);
                puppy_vmenu_tex=render_track(puppy_device,4,gui__u32(p));
                if !puppy_vmenu_tex {return 0;}
            }
            render_gl(puppy_device,"glUseProgram",puppy_toolbar_program,0,0,0,0,0);
            render_gl(puppy_device,"glActiveTexture",0x84c0,0,0,0,0,0);
            render_gl(puppy_device,"glBindTexture",0x0de1,puppy_vmenu_tex,0,0,0,0);
            puppy_ui_upload(rgb,nw,nh);
            let x0=nx*pw/w;let x1=(nx+nw)*pw/w;let y0=ph-(ny+nh)*ph/h;let y1=ph-ny*ph/h;
            render_gl(puppy_device,"glViewport",x0,y0,x1-x0,y1-y0,0,0);
            render_gl(puppy_device,"glDisable",0x0b71,0,0,0,0,0);
            render_gl(puppy_device,"glDisable",0x0b44,0,0,0,0,0);
            return render_draw(puppy_device,puppy_toolbar_vao,4,0,3);
        }
        if !puppy_toolbar_hover_id || puppy_reader_report {return 1;}
        let info=puppy_toolbar_hover_info(cad_project_open(puppy_history+24),w,puppy_pointer_x,puppy_pointer_y);
        if !info {return 1;}
        let text=load64(info+16);let cx=load64(info+8);
        let tw=puppy_limit(puppy_toolbar_text_width(text)+20,64,w-16);
        let th=26;let tx=puppy_limit(cx-tw/2,8,w-tw-8);
        let ty=74;if load64(info)>=100 {ty=38;}
        let rgb=m_take(tw*th*4);
        puppy_ui_rect(rgb,tw,th,0,0,tw,th,92,134,140);
        puppy_ui_rect(rgb,tw,th,1,1,tw-2,th-2,24,34,37);
        puppy_ui_text(rgb,tw,th,10,18,text,tw-8);
        if !puppy_vmenu_tex {
            let p=m_take(8);store64(p,0);render_gl(puppy_device,"glGenTextures",1,p,0,0,0,0);
            puppy_vmenu_tex=render_track(puppy_device,4,gui__u32(p));
            if !puppy_vmenu_tex {return 0;}
        }
        render_gl(puppy_device,"glUseProgram",puppy_toolbar_program,0,0,0,0,0);
        render_gl(puppy_device,"glActiveTexture",0x84c0,0,0,0,0,0);
        render_gl(puppy_device,"glBindTexture",0x0de1,puppy_vmenu_tex,0,0,0,0);
        puppy_ui_upload(rgb,tw,th);
        let x0=tx*pw/w;let x1=(tx+tw)*pw/w;let y0=ph-(ty+th)*ph/h;let y1=ph-ty*ph/h;
        render_gl(puppy_device,"glViewport",x0,y0,x1-x0,y1-y0,0,0);
        render_gl(puppy_device,"glDisable",0x0b71,0,0,0,0,0);
        render_gl(puppy_device,"glDisable",0x0b44,0,0,0,0,0);
        return render_draw(puppy_device,puppy_toolbar_vao,4,0,3);
    }
    let project=cad_project_open(puppy_history+24);
    let rows=puppy_vmenu_entries(project);let count=cg_count(rows);
    if !count {puppy_vmenu_close();return 1;}
    let window=load64(puppy_device);let w=gui_width(window);let h=gui_height(window);
    let mw=puppy_limit(PUPPY_VMENU_WIDTH,1,w-8);
    let mh=count*PUPPY_VMENU_ROW+2;
    let rgb=m_take(mw*mh*4);
    puppy_ui_rect(rgb,mw,mh,0,0,mw,mh,89,111,109);
    let i=0;while i<count {
        let top=1+i*PUPPY_VMENU_ROW;let label=load64(cg_at(rows,i));
        if i==puppy_vmenu_choice {
            puppy_ui_rect(rgb,mw,mh,1,top,mw-2,PUPPY_VMENU_ROW,53,76,72);
            puppy_ui_rect(rgb,mw,mh,1,top+2,3,PUPPY_VMENU_ROW-4,143,217,184);
        }else {
            puppy_ui_rect(rgb,mw,mh,1,top,mw-2,PUPPY_VMENU_ROW,30,43,47);
        }
        puppy_ui_rect(rgb,mw,mh,1,top+PUPPY_VMENU_ROW-1,mw-2,1,44,62,65);
        puppy_ui_text(rgb,mw,mh,12,top+18,label,mw-8);
        i=i+1;
    }
    if !puppy_vmenu_tex {
        let p=m_take(8);store64(p,0);render_gl(puppy_device,"glGenTextures",1,p,0,0,0,0);
        puppy_vmenu_tex=render_track(puppy_device,4,gui__u32(p));
        if !puppy_vmenu_tex {return 0;}
    }
    render_gl(puppy_device,"glUseProgram",puppy_toolbar_program,0,0,0,0,0);
    render_gl(puppy_device,"glActiveTexture",0x84c0,0,0,0,0,0);
    render_gl(puppy_device,"glBindTexture",0x0de1,puppy_vmenu_tex,0,0,0,0);
    puppy_ui_upload(rgb,mw,mh);
    let x0=puppy_vmenu_x*pw/w;let x1=(puppy_vmenu_x+mw)*pw/w;
    let y0=ph-(puppy_vmenu_y+mh)*ph/h;let y1=ph-puppy_vmenu_y*ph/h;
    render_gl(puppy_device,"glViewport",x0,y0,x1-x0,y1-y0,0,0);
    render_gl(puppy_device,"glDisable",0x0b71,0,0,0,0,0);
    render_gl(puppy_device,"glDisable",0x0b44,0,0,0,0,0);
    return render_draw(puppy_device,puppy_toolbar_vao,4,0,3);
}
