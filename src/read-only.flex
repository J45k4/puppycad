// The viewer accepts navigation events only. It holds the original file bytes
// for source inspection independently of the model used for a partial preview.
global puppy_reader_report=0;
global puppy_reader_text=0;
global puppy_reader_source=0;
global puppy_reader_preview=0;
global puppy_reader_line=0;
global puppy_reader_offsets=0;
global puppy_reader_count=0;
global puppy_reader_columns=0;
global puppy_reader_font=0;

fn puppy_reader_name() {
    let base=puppy_path;let i=0;
    while load8(puppy_path+i) {if load8(puppy_path+i)==47 {base=puppy_path+i+1;}i=i+1;}
    return base;
}
fn puppy_reader_reason() {
    let report=puppy_reader_report;let reason=j_string(report,"readOnly");
    let id=j_string(report,"nodeId");if !m_len(id) {id=j_string(report,"itemId");}
    if m_len(id) {return m_cat(reason,m_cat("  (",m_cat(id,")")));}
    return reason;
}
fn puppy_reader_view_name() {if puppy_reader_preview==2 {return "Saved sketch";}return "Geometry preview";}
fn puppy_reader_status() {
    let mode=puppy_reader_view_name();if puppy_reader_source {mode="File source";}
    let title=m_cat(puppy_reader_name(),m_cat("  |  Read-only  |  ",m_cat(mode,m_cat("  |  ",puppy_reader_reason()))));
    if puppy_pending_title {puppy_drop(puppy_pending_title);}puppy_pending_title=puppy_snapshot(title);
    puppy_toolbar_dirty=1;return 1;
}
fn puppy_reader_begin(report,source) {
    puppy_reader_report=report;puppy_reader_text=source;
    puppy_reader_font=tf_font_open(puppy_toolbar_context,"monospace",14,0,0);
    if !puppy_reader_font {return gui__fail(tf_error);}
    let mark=m_mark();let mesh=cad_project_read_only_mesh(report);
    if pc_first(j_get(mesh,"vertices")) && !j_get(mesh,"error") {
        puppy_reader_preview=puppy_upload(j_get(report,"document"),mesh);
        if !puppy_reader_preview {m_reset(mark);return 0;}
    }
    let doc=j_get(report,"document");
    if !puppy_reader_preview && j_integer(doc,"version")==4 && !cad_project_is_studio(doc) {
        let nodes=pc_nodes(cad_project_part(doc));let sketch=pc_first(nodes);let index=0;
        while sketch && !puppy_reader_preview {
            if m_eq(j_string(sketch,"type"),"sketch") {
                cg_begin();
                if cg_count(cad_sketch_paths(nodes,j_string(sketch,"id"))) {
                    puppy_draft_index=index;
                    if puppy_fit_sketch(doc,0) {puppy_reader_preview=2;}
                }
                index=index+1;
            }
            sketch=load64(sketch+24);
        }
    }
    m_reset(mark);cg_begin();puppy_reader_source=!puppy_reader_preview;puppy_empty=!puppy_reader_preview;
    puppy_sketch=puppy_reader_preview==2;puppy_draft=puppy_sketch;
    return puppy_reader_status();
}
fn puppy_reader_bar(rgb,w) {
    puppy_bar_rect(rgb,w,0,0,w,76,30,43,47);
    puppy_bar_text(rgb,w,12,25,"Read-only",92);puppy_bar_text(rgb,w,103,25,puppy_reader_reason(),w-8);
    if puppy_reader_preview {
        let switch="File source";if puppy_reader_source {switch="Preview";}
        puppy_bar_button(rgb,w,12,92,switch);
    }
    let x=12;if puppy_reader_preview {x=112;}
    if puppy_reader_source {
        puppy_bar_button(rgb,w,x,65,"PgUp");puppy_bar_button(rgb,w,x+73,65,"PgDn");
        let range=m_cat(m_int(puppy_reader_line+1),m_cat(" / ",m_int(puppy_reader_count)));
        puppy_bar_text(rgb,w,x+148,60,range,w-8);
    }else {
        let minus="Rotate −";let plus="Rotate +";if puppy_reader_preview==2 {minus="Zoom −";plus="Zoom +";}
        puppy_bar_button(rgb,w,x,72,minus);puppy_bar_button(rgb,w,x+80,72,plus);
        puppy_bar_text(rgb,w,x+163,60,puppy_reader_view_name(),w-8);
    }
    puppy_bar_rect(rgb,w,0,75,w,1,73,96,95);return 1;
}
fn puppy_reader_index(w) {
    // Store the first byte of each wrapped row. Never split a UTF-8 sequence.
    let cols=puppy_limit((w-26)/10,4,512);if cols==puppy_reader_columns {return 1;}
    let starts=m_buffer();let word=m_take(8);let i=0;let col=0;store64(word,0);m_append(starts,word,8);
    let source=puppy_reader_text;
    while load8(source+i) {
        let c=load8(source+i);let width=1;if c==9 {width=4;}else if c>=128 {width=2;}
        if c!=10 && col && col+width>cols {store64(word,i);m_append(starts,word,8);col=0;}
        i=i+1;while (load8(source+i)&192)==128 {i=i+1;}
        col=col+width;
        if c==10 {store64(word,i);m_append(starts,word,8);col=0;}
    }
    if puppy_reader_offsets {syscall(11,puppy_reader_offsets,(puppy_reader_count+1)*8,0,0,0,0);}
    puppy_reader_count=load64(starts+8)/8;store64(word,i);m_append(starts,word,8);
    puppy_reader_offsets=alloc(load64(starts+8));m_assert(puppy_reader_offsets>0,"Cannot allocate source line index");
    m_copy(puppy_reader_offsets,m_data(starts),load64(starts+8));puppy_reader_columns=cols;return 1;
}
fn puppy_reader_page() {return puppy_limit((gui_height(load64(puppy_device))-86)/21,1,800);}
fn puppy_reader_scroll(delta) {
    puppy_reader_line=puppy_limit(puppy_reader_line+delta,0,puppy_limit(puppy_reader_count-puppy_reader_page(),0,puppy_reader_count));
    return 1;
}
fn puppy_reader_draw_source(w,h) {
    let window=load64(puppy_device);let width=gui_width(window);let height=gui_height(window);
    if width<1 || height<1 {return 1;}
    puppy_reader_index(width);puppy_reader_scroll(0);
    let pixels=m_take(width*height*4);puppy_ui_rect(pixels,width,height,0,0,width,height,24,34,38);
    // The ordinary toolbar writes a 76-pixel strip.
    let bar=m_take(width*76*4);puppy_reader_bar(bar,width);
    m_copy(pixels,bar,width*puppy_limit(height,0,76)*4);
    let i=puppy_reader_line;let y=95;
    while y<=height && i<puppy_reader_count {
        let start=load64(puppy_reader_offsets+i*8);let end=load64(puppy_reader_offsets+(i+1)*8);
        let text=m_buffer();while start<end {
            let c=load8(puppy_reader_text+start);
            if c==9 {m_text(text,"    ");}else if c!=13 && c!=10 {m_append(text,puppy_reader_text+start,1);}
            start=start+1;
        }
        puppy_ui_font_text(pixels,width,height,12,y,m_data(text),width-5,puppy_reader_font);
        i=i+1;y=y+21;
    }
    if !render_begin(puppy_device,0,w,h) {return 0;}
    render_gl(puppy_device,"glUseProgram",puppy_toolbar_program,0,0,0,0,0);
    render_gl(puppy_device,"glActiveTexture",0x84c0,0,0,0,0,0);
    render_gl(puppy_device,"glBindTexture",0x0de1,puppy_toolbar_texture,0,0,0,0);
    puppy_ui_upload(pixels,width,height);puppy_toolbar_dirty=1;
    render_gl(puppy_device,"glViewport",0,0,w,h,0,0);
    render_gl(puppy_device,"glDisable",0x0b71,0,0,0,0,0);render_gl(puppy_device,"glDisable",0x0b44,0,0,0,0,0);
    return render_draw(puppy_device,puppy_toolbar_vao,4,0,3) && gui_gl_swap(window);
}
fn puppy_reader_sketch_zoom(steps) {
    let step=puppy_limit(puppy_draft_scale/4,1,8000);
    puppy_draft_scale=puppy_limit(puppy_draft_scale+step*steps,1,32000);return 1;
}
fn puppy_reader_event(event) {
    let kind=gui_event_kind(event);let key=gui_event_code(event);
    if kind==GUI_CLOSE || kind==GUI_QUIT {return 0;}
    if kind==GUI_RESIZE || kind==GUI_EXPOSE {return 2;}
    if kind==GUI_WHEEL {
        if !puppy_reader_source && puppy_reader_preview==2 {puppy_reader_sketch_zoom(gui_event_dy(event));}
        else if puppy_reader_source {puppy_reader_scroll(-3*gui_event_dy(event));}
        else {puppy_zoom=puppy_limit(puppy_zoom-gui_event_dy(event)*15,120,800);}
        return 2;
    }
    if kind==GUI_MOUSE_DOWN && gui_event_button(event)==1 && gui_event_y(event)>=42 && gui_event_y(event)<68 {
        let x=gui_event_x(event);let left=12;if puppy_reader_preview {left=112;}
        if puppy_reader_preview && x>=12 && x<104 {key=43;}
        else {
            let size=72;if puppy_reader_source {size=65;}
            if x>=left && x<left+size {key=75;}
            else if x>=left+size+8 && x<left+2*size+8 {key=78;}else {return 1;}
        }
    }else if kind!=GUI_KEY_DOWN {return 1;}
    if key==GUI_KEY_ESCAPE {return 0;}
    if gui_event_modifiers(event)&0x3c0 {return 1;}
    if key==43 && puppy_reader_preview {puppy_reader_source=!puppy_reader_source;puppy_reader_status();return 2;}
    if puppy_reader_source {
        if key==75 {puppy_reader_scroll(-puppy_reader_page());}else if key==78 {puppy_reader_scroll(puppy_reader_page());}
        else if key==74 {puppy_reader_scroll(-puppy_reader_count);}else if key==77 {puppy_reader_scroll(puppy_reader_count);}
        else if key==82 {puppy_reader_scroll(-1);}else if key==81 {puppy_reader_scroll(1);}else {return 1;}
    }else if puppy_reader_preview==2 {
        if key==74 {cg_begin();puppy_fit_sketch(j_get(puppy_reader_report,"document"),0);}
        else if key==75 || key==81 {puppy_reader_sketch_zoom(-1);}
        else if key==78 || key==82 {puppy_reader_sketch_zoom(1);}else {return 1;}
    }else {
        if key==75 || key==80 {puppy_camera_rotate(-15000,0);}else if key==78 || key==79 {puppy_camera_rotate(15000,0);}
        else if key==82 {puppy_camera_rotate(0,10000);}
        else if key==81 {puppy_camera_rotate(0,-10000);}
        else if key==74 {puppy_zoom=280;puppy_camera_set_angles(45000,35264,0);}else {return 1;}
    }
    return 2;
}
