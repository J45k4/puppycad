// The SDK returns paper coordinates in tenths of a millimetre, with Y up.
global puppy_sheet_scene=0;
global puppy_sheet_texture=0;
global puppy_sheet_width=0;
global puppy_sheet_height=0;

fn puppy_sheet_open(project,id) {
    let input=j_object();
    j_add(input,"document",j_parse(j_encode(project)));
    cg_j_text(input,"sheet_id",id);
    let scene=pdd_interactive(input);
    if j_get(scene,"error") {return puppy_title(project,j_string(scene,"error"));}
    if puppy_sheet_scene {puppy_drop(puppy_sheet_scene);}
    puppy_sheet_scene=puppy_snapshot(j_encode(scene));
    puppy_sheet_width=0;
    return puppy_title(project,m_cat("Drawing sheet  ",j_string(scene,"name")));
}

fn puppy_sheet_close() {
    if !puppy_sheet_scene {return 0;}
    puppy_drop(puppy_sheet_scene);puppy_sheet_scene=0;puppy_toolbar_dirty=1;
    return 1;
}

fn puppy_sheet_actions() {
    let rows=cg_array();
    puppy_compact_button(rows,"Back to model",GUI_KEY_ESCAPE,0);
    puppy_compact_button(rows,"Export PDF",19,192);
    return rows;
}

fn puppy_sheet_x(paper,x) {return load64(paper)+x*load64(paper+16)/2970;}
fn puppy_sheet_y(paper,y) {return load64(paper+8)+(2100-y)*load64(paper+16)/2970;}

// Integer rasterization into the same RGBA surface used for native text.
fn puppy_sheet_line(rgb,w,h,a,b,shade,dash) {
    let x=load64(a);let y=load64(a+8);let u=load64(b);let v=load64(b+8);
    let dx=cg_abs(u-x);let dy=-cg_abs(v-y);let sx=1;let sy=1;
    if x>u {sx=-1;}
    if y>v {sy=-1;}
    let err=dx+dy;let i=0;
    while 1 {
        if x>=0 && x<w && y>=0 && y<h && (!dash || i%6<3) {
            let pixel=rgb+(y*w+x)*4;
            store8(pixel,shade);store8(pixel+1,shade+8);store8(pixel+2,shade+12);
        }
        if x==u && y==v {return 1;}
        let step=err*2;
        if step>=dy {err=err+dy;x=x+sx;}
        if step<=dx {err=err+dx;y=y+sy;}
        i=i+1;
    }
}
fn puppy_sheet_point(paper,point) {
    let x=pc_first(point);
    return cg_v(puppy_sheet_x(paper,j_value(x)),puppy_sheet_y(paper,j_value(load64(x+24))),0);
}
fn puppy_sheet_paths(rgb,w,h,paper,paths,shade,dash) {
    let line=pc_first(paths);
    while line {
        let a=pc_first(line);let b=load64(a+24);let c=load64(b+24);let d=load64(c+24);
        let p=cg_v(puppy_sheet_x(paper,j_value(a)),puppy_sheet_y(paper,j_value(b)),0);
        let q=cg_v(puppy_sheet_x(paper,j_value(c)),puppy_sheet_y(paper,j_value(d)),0);
        puppy_sheet_line(rgb,w,h,p,q,shade,dash);
        line=load64(line+24);
    }
}
fn puppy_sheet_text(rgb,w,h,x,y,label,right) {
    return puppy_ui_tinted_text(rgb,w,h,x,y,label,right,puppy_toolbar_font,38);
}

fn puppy_sheet_annotations(rgb,w,h,paper,scene) {
    let detail=pc_first(j_get(scene,"detail_callouts"));
    while detail {
        let a=puppy_sheet_point(paper,j_get(detail,"a"));let b=puppy_sheet_point(paper,j_get(detail,"b"));
        let c=cg_v(load64(a),load64(b+8),0);let d=cg_v(load64(b),load64(a+8),0);
        puppy_sheet_line(rgb,w,h,a,c,104,1);puppy_sheet_line(rgb,w,h,c,b,104,1);
        puppy_sheet_line(rgb,w,h,b,d,104,1);puppy_sheet_line(rgb,w,h,d,a,104,1);
        puppy_sheet_text(rgb,w,h,load64(c),load64(c+8)-5,pdd_detail_label(j_integer(detail,"number")),w-8);
        detail=load64(detail+24);
    }
    let dimension=pc_first(j_get(scene,"dimensions"));
    while dimension {
        if j_get(dimension,"a") && j_get(dimension,"b") {
            let a=puppy_sheet_point(paper,j_get(dimension,"span_a"));let b=puppy_sheet_point(paper,j_get(dimension,"span_b"));
            puppy_sheet_line(rgb,w,h,puppy_sheet_point(paper,j_get(dimension,"a")),a,99,0);
            puppy_sheet_line(rgb,w,h,puppy_sheet_point(paper,j_get(dimension,"b")),b,99,0);
            puppy_sheet_line(rgb,w,h,a,b,99,1);
            puppy_sheet_text(rgb,w,h,(load64(a)+load64(b))/2+4,(load64(a+8)+load64(b+8))/2-5,m_int(j_integer(dimension,"number")),w-8);
        }
        dimension=load64(dimension+24);
    }
    let left=cg_array();let right=cg_array();
    let balloon=pc_first(j_get(scene,"balloons"));
    while balloon {
        let label=j_get(balloon,"label_at");
        if label {
            let side=right;if j_value(pc_first(label))<1485 {side=left;}
            cg_push(side,cg_v(balloon,puppy_sheet_point(paper,label),0));
        }
        balloon=load64(balloon+24);
    }
    puppy_sheet_balloon_rail(rgb,w,h,paper,left);
    puppy_sheet_balloon_rail(rgb,w,h,paper,right);
}

fn puppy_sheet_balloon_rail(rgb,w,h,paper,balloons) {
    let count=cg_count(balloons);let i=0;
    let low=puppy_sheet_y(paper,1720)+10;
    let high=puppy_sheet_y(paper,700)-10;
    let gap=19;
    if count>1 {gap=puppy_limit((high-low)/(count-1),1,19);}
    let previous=low-gap;
    // Paper spacing can become smaller than the text at compact window sizes.
    // Pack in pixel space while preserving the order of leaders on each rail.
    while i<count {
        let next=0;let j=0;
        while j<count {
            let row=cg_at(balloons,j);
            if !load64(row+16) && (!next || load64(load64(row+8)+8)<load64(load64(next+8)+8)) {next=row;}
            j=j+1;
        }
        let balloon=load64(next);let label=load64(next+8);
        let x=load64(label);let y=puppy_limit(load64(label+8),previous+gap,high-(count-i-1)*gap);
        puppy_sheet_line(rgb,w,h,puppy_sheet_point(paper,j_get(balloon,"anchor")),cg_v(x,y,0),102,0);
        if gap>=18 {
            puppy_ui_rect(rgb,w,h,x-11,y-9,22,18,67,90,107);
            puppy_ui_text(rgb,w,h,x-8,y+5,m_int(j_integer(balloon,"item_number")),x+11);
        }
        previous=y;store64(next+16,1);i=i+1;
    }
}

fn puppy_sheet_legend(rgb,w,h,scene,x,top,right) {
    let legend=pc_first(j_get(scene,"dimensions"));let items=pc_first(j_get(scene,"balloons"));
    let both=legend && items;let middle=right;if both {middle=(x+right)/2;}
    let y=top;
    while legend {
        let text=m_cat(m_int(j_integer(legend,"number")),m_cat("  ",j_string(legend,"name")));
        puppy_sheet_text(rgb,w,h,x,y,text,middle);
        puppy_sheet_text(rgb,w,h,x+10,y+17,j_string(legend,"label"),middle);
        y=y+34;legend=load64(legend+24);
    }
    if both {x=middle+12;}
    y=top;
    while items {
        let text=j_string(items,"problem");
        if !m_len(text) {text=m_cat(m_int(j_integer(items,"item_number")),m_cat("  ",j_string(items,"name")));}
        puppy_sheet_text(rgb,w,h,x,y,text,right);y=y+17;items=load64(items+24);
    }
}

fn puppy_sheet_build(w,h,scene) {
    let rgb=m_take(w*h*4);puppy_ui_rect(rgb,w,h,0,0,w,h,20,29,33);
    // A fixed-aspect projection region uses the SDK's paper coordinates.
    // The dimension and parts lists keep a consistent font size as the window resizes.
    let rows=cg_j_count(j_get(scene,"dimensions"))*34;
    let parts=cg_j_count(j_get(scene,"balloons"))*17;if parts>rows {rows=parts;}
    let page_top=10;if !puppy_design_open {page_top=36;}
    let width=w-24;
    let fit=(h-72-page_top-rows)*2970/1050;if width>fit {width=fit;}
    if width<40 {
        puppy_ui_text(rgb,w,h,12,26,"Enlarge the window to see the sheet",w-8);
        return puppy_ui_upload(rgb,w,h);
    }
    let height=width*1050/2970;let x=(w-width)/2;let y=page_top+50;
    let paper=cg_v(x,y+(1730-2100)*width/2970,width);
    puppy_ui_rect(rgb,w,h,x,page_top,width,55+height+rows+10,246,247,245);
    puppy_sheet_text(rgb,w,h,x+12,page_top+23,j_string(scene,"name"),x+width-8);
    let axis=j_string(scene,"axis");let view="Isometric";
    if m_eq(axis,"top") {view="Top";}
    if m_eq(axis,"front") {view="Front";}
    if m_eq(axis,"right") {view="Right";}
    let label=m_cat(view,m_cat("   Scale ",pdd_scale_label(scene)));
    let section=j_get(scene,"section_mm");
    if section {label=m_cat(label,m_cat("   Section ",m_cat(cg_decimal(cg_number(section))," mm")));}
    else if j_integer(scene,"exploded") {label=m_cat(label,"   Exploded assembly");}
    if j_get(scene,"detail_parent_id") {label=m_cat(label,m_cat("   Detail of ",j_string(scene,"detail_parent_name")));}
    puppy_sheet_text(rgb,w,h,x+12,page_top+42,label,x+width-8);
    puppy_sheet_paths(rgb,w,h,paper,j_get(scene,"exploded_guides"),145,1);
    puppy_sheet_paths(rgb,w,h,paper,j_get(scene,"hidden_lines"),145,1);
    puppy_sheet_paths(rgb,w,h,paper,j_get(scene,"lines"),49,0);
    puppy_sheet_annotations(rgb,w,h,paper,scene);
    if rows {
        let top=y+height+7;puppy_ui_rect(rgb,w,h,x+10,top,width-20,1,178,188,184);
        puppy_sheet_legend(rgb,w,h,scene,x+12,top+17,x+width-10);
    }
    return puppy_ui_upload(rgb,w,h);
}

fn puppy_sheet_draw(w,h) {
    let window=load64(puppy_device);let win_w=gui_width(window);let win_h=gui_height(window);
    let sidebar=0;if puppy_design_open {sidebar=puppy_design_width();}
    let width=win_w-sidebar;let height=win_h-76;
    if width<=0 || height<=0 {return 1;}
    if !puppy_sheet_texture {
        let name=m_take(8);store64(name,0);render_gl(puppy_device,"glGenTextures",1,name,0,0,0,0);
        puppy_sheet_texture=render_track(puppy_device,4,gui__u32(name));if !puppy_sheet_texture {return 0;}
    }
    render_gl(puppy_device,"glBindTexture",0x0de1,puppy_sheet_texture,0,0,0,0);
    if puppy_sheet_width!=width || puppy_sheet_height!=height {
        puppy_sheet_build(width,height,j_parse(puppy_sheet_scene+24));puppy_sheet_width=width;puppy_sheet_height=height;
    }
    let left=sidebar*w/win_w;
    render_gl(puppy_device,"glViewport",left,0,w-left,h-76*h/win_h,0,0);
    return render_draw(puppy_device,puppy_toolbar_vao,4,0,3);
}

fn puppy_sheet_event(event) {
    // Keep model picking and cameras inactive until the sheet closes.
    let kind=gui_event_kind(event);
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 {
        let y=gui_event_y(event);let x=gui_event_x(event);
        if y>=42 && y<68 {puppy_action_bar(puppy_sheet_actions(),0,gui_width(load64(puppy_device)),x);}
    }
    return kind==GUI_MOUSE_DOWN || kind==GUI_MOUSE_UP || kind==GUI_MOUSE_MOVE || kind==GUI_WHEEL;
}
