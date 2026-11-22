// Native text and hit targets are composed in logical window pixels. The
// toolbar texture is uploaded when its content or window width changes.
import "gui/text/font.flex";
global puppy_toolbar_font=0;
global puppy_toolbar_context=0;
global puppy_toolbar_program=0;
global puppy_toolbar_vao=0;
global puppy_toolbar_texture=0;
global puppy_toolbar_width=0;
global puppy_toolbar_dirty=1;
global puppy_status=0;
global puppy_bar_page=0;
global puppy_breadcrumb_assembly_left=0;
global puppy_breadcrumb_assembly_right=0;
global puppy_breadcrumb_part_left=0;
global puppy_breadcrumb_part_right=0;

fn puppy_toolbar_open() {
    puppy_toolbar_context=tf_context();
    if !puppy_toolbar_context {return gui__fail(tf_error);}
    puppy_toolbar_font=tf_font_open(puppy_toolbar_context,"sans-serif",15,0,0);
    if !puppy_toolbar_font {return gui__fail(tf_error);}
    let v="#version 450 core\nout vec2 uv;void main(){vec2 p=vec2((gl_VertexID<<1)&2,gl_VertexID&2);uv=vec2(p.x,1.0-p.y);gl_Position=vec4(p*2.0-1.0,0.0,1.0);}";
    let f="#version 450 core\nin vec2 uv;uniform sampler2D bar;out vec4 color;void main(){color=texture(bar,uv);}";
    puppy_toolbar_program=render_program(puppy_device,v,f);
    puppy_toolbar_vao=render_array(puppy_device);
    let p=m_take(8);store64(p,0);render_gl(puppy_device,"glGenTextures",1,p,0,0,0,0);
    puppy_toolbar_texture=render_track(puppy_device,4,gui__u32(p));
    return puppy_toolbar_program && puppy_toolbar_vao && puppy_toolbar_texture;
}
fn puppy_bar_rect(rgb,w,x,y,width,height,r,g,b) {
    return puppy_ui_rect(rgb,w,76,x,y,width,height,r,g,b);
}
fn puppy_ui_rect(rgb,w,h,x,y,width,height,r,g,b) {
    let row=y;if row<0 {row=0;}while row<y+height && row<h {
        let col=x;if col<0 {col=0;}while col<x+width && col<w {
            let pixel=rgb+(row*w+col)*4;
            store8(pixel,r);store8(pixel+1,g);store8(pixel+2,b);store8(pixel+3,255);
            col=col+1;
        }
        row=row+1;
    }
    return 0;
}
fn puppy_toolbar_text_width(text) {
    let shape=tf_shape(puppy_toolbar_font,text,m_len(text),0);
    if !shape {return 0;}
    let width=0;let i=0;
    while i<load64(shape) {width=width+load64(shape+64+i*48+16);i=i+1;}
    tf_shape_free(shape);
    return (width+63)/64;
}
fn puppy_bar_text(rgb,w,x,baseline,text,right) {
    return puppy_ui_text(rgb,w,76,x,baseline,text,right);
}
fn puppy_ui_text(rgb,w,h,x,baseline,text,right) {
    return puppy_ui_font_text(rgb,w,h,x,baseline,text,right,puppy_toolbar_font);
}
fn puppy_ui_font_text(rgb,w,h,x,baseline,text,right,font) {
    return puppy_ui_tinted_text(rgb,w,h,x,baseline,text,right,font,236);
}
fn puppy_ui_tinted_text(rgb,w,h,x,baseline,text,right,font,tone) {
    let shape=tf_shape(font,text,m_len(text),0);
    if !shape {return 0;}
    let i=0;let pen=x*64;
    while i<load64(shape) && pen/64<right {
        let glyph=shape+64+i*48;let slot=tf_glyph_bitmap(font,load64(glyph));
        if slot {
            let bitmap=slot+152;let cols=bytes_load32(bitmap+4);let rows=bytes_load32(bitmap);
            let left=(pen+load64(glyph+32))/64+tf_i32(slot+192);
            let top=baseline-load64(glyph+40)/64-tf_i32(slot+196);let dy=0;
            while dy<rows && load8(bitmap+26)==2 {
                let dx=0;while dx<cols {
                    let px=left+dx;let py=top+dy;
                    if px>=0 && px>=x && px<w && px<right && py>=0 && py<h {
                        let alpha=load8(load64(bitmap+16)+dy*tf_i32(bitmap+8)+dx);
                        let dst=rgb+(py*w+px)*4;let k=0;
                        while k<3 {store8(dst+k,(load8(dst+k)*(255-alpha)+tone*alpha)/255);k=k+1;}
                    }
                    dx=dx+1;
                }
                dy=dy+1;
            }
        }
        pen=pen+load64(glyph+16);i=i+1;
    }
    tf_shape_free(shape);return 1;
}
fn puppy_bar_button(rgb,w,x,width,label) {
    puppy_bar_rect(rgb,w,x,42,width,26,48,68,70);
    return puppy_bar_text(rgb,w,x+8,60,label,x+width);
}
// Blend 16×16 masks into the toolbar texture. The marks . + # encode increasing coverage.
fn puppy_bar_icon(rgb,w,x,y,mask,tone,mirror) {
    let row=0;while row<16 {
        let line=cg_at(mask,row);let col=0;while col<16 && x+col<w {
            let sample=col;if mirror {sample=15-col;}
            let mark=load8(line+sample);
            let coverage=0;
            if mark==46 {coverage=1;}else if mark==43 {coverage=2;}else if mark==35 {coverage=3;}
            if coverage && x+col>=0 {
                let pixel=rgb+((y+row)*w+x+col)*4;
                let channel=0;while channel<3 {
                    store8(pixel+channel,(load8(pixel+channel)*(3-coverage)+tone*coverage)/3);
                    channel=channel+1;
                }
            }
            col=col+1;
        }
        row=row+1;
    }
}
fn puppy_bar_history(rgb,w,x,width,redo) {
    let mask=cg_array();
    cg_push(mask,"                ");
    cg_push(mask,"                ");
    cg_push(mask,"      +.        ");
    cg_push(mask,"     ##.        ");
    cg_push(mask,"   .###+..      ");
    cg_push(mask,"  .########+.   ");
    cg_push(mask," +###########.  ");
    cg_push(mask," +############  ");
    cg_push(mask,"  .###########+ ");
    cg_push(mask,"   .###.   .### ");
    cg_push(mask,"     +#.     +# ");
    cg_push(mask,"      +.      # ");
    cg_push(mask,"              . ");
    cg_push(mask,"                ");
    cg_push(mask,"                ");
    cg_push(mask,"                ");
    let offset=0;if redo {offset=8;}
    let tone=126;if load64(puppy_history+offset) {tone=236;}
    puppy_bar_rect(rgb,w,x,42,width,26,48,68,70);
    return puppy_bar_icon(rgb,w,x+(width-16)/2,48,mask,tone,redo);
}
fn puppy_bar_sketch(rgb,w,x,width,label) {
    let mask=cg_array();
    cg_push(mask,"                ");
    cg_push(mask,"          +##.  ");
    cg_push(mask,"         +#++#. ");
    cg_push(mask,"        +#+#.+# ");
    cg_push(mask,"       +#. +#+# ");
    cg_push(mask,"      +#.   ##. ");
    cg_push(mask,"     +#.   +#.  ");
    cg_push(mask,"    +#.   +#.   ");
    cg_push(mask,"   +#.   +#.    ");
    cg_push(mask,"  .#.   +#.     ");
    cg_push(mask,"  ++   +#.      ");
    cg_push(mask,"  #. .+#.       ");
    cg_push(mask," .####+.        ");
    cg_push(mask," +#+.           ");
    cg_push(mask,"                ");
    cg_push(mask,"                ");
    puppy_bar_rect(rgb,w,x,42,width,26,48,68,70);
    puppy_bar_icon(rgb,w,x+5,47,mask,236,0);
    puppy_bar_rect(rgb,w,x+5,62,15,1,143,217,184);
    return puppy_bar_text(rgb,w,x+25,60,label,x+width-4);
}
fn puppy_value_button_hit(project,x) {
    if puppy_draft || puppy_empty {return 0;}
    let left=12;
    if puppy_isolated {left=left+96;}
    if !puppy_sketch {left=left+86;}
    return x>=left && x<left+76 && puppy_active_field(project);
}
fn puppy_action_bar(rows,rgb,w,click) {
    let count=cg_count(rows);if !count {return 1;}let width=puppy_limit((w-16)/count-8,1,112);
    let x=12;let i=0;
    while i<cg_count(rows) {
        let entry=cg_at(rows,i);
        if rgb {puppy_bar_button(rgb,w,x,width,load64(entry));}
        else if click>=x && click<x+width {return puppy_command(load64(entry+8),load64(entry+16));}
        x=x+width+8;i=i+1;
    }
    return 1;
}
fn puppy_loft_actions() {
    let rows=cg_array();
    puppy_compact_button(rows,"Next sketch",43,0);puppy_compact_button(rows,"Next profile",19,0);
    if puppy_loft_first {
        puppy_compact_button(rows,"Create loft",40,0);puppy_compact_button(rows,"Join loft",40,3);
    }else {puppy_compact_button(rows,"Use profile",40,0);}
    puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);return rows;
}
fn puppy_line_button(project) {
    let nodes=pc_nodes(cad_project_part(project));let sketch=puppy_draft_sketch(project);
    if puppy_draft {return !puppy_draft_on_face(project) && sketch && pc_line_entities(nodes,sketch);}
    let row=pc_first(nodes);while row {
        if m_eq(j_string(row,"type"),"sketch") && pc_line_entities(nodes,row) {return 1;}
        row=load64(row+24);
    }
    return 0;
}
fn puppy_bar_breadcrumb(rgb,w,x,right,label,pad) {
    let end=x+puppy_limit(puppy_toolbar_text_width(label)+pad*2,0,right-x);
    if puppy_pointer_x>=x && puppy_pointer_x<end && puppy_pointer_y>=6 && puppy_pointer_y<34 {
        puppy_bar_rect(rgb,w,x,6,end-x,28,45,64,67);
    }else {
        puppy_bar_rect(rgb,w,x,6,end-x,28,37,53,56);
    }
    puppy_bar_text(rgb,w,x+pad,25,label,end-pad);
    puppy_bar_rect(rgb,w,x,29,end-x,1,143,217,184);
    return end;
}
fn puppy_workspace_toolbar_compact(project,w) {
    if w<800 {return 1;}
    let part=cad_project_part(project);if !part {return 0;}
    let assembly=puppy_workspace_parent(project,part);
    if !assembly && !puppy_sketch {return 0;}
    // Use the compact row when the toolbar controls crowd out the ancestor links.
    let path_width=puppy_toolbar_text_width(puppy_workspace_heading(project,1))+4;
    let divider=puppy_toolbar_text_width(" › ");
    if assembly {path_width=path_width+puppy_toolbar_text_width("Assembly")+divider;}
    if puppy_sketch {path_width=path_width+puppy_toolbar_text_width("Part")+divider;}
    let start=12;
    if puppy_isolated {start=start+96;}
    if !puppy_sketch && !puppy_empty {start=start+86;}
    if puppy_draft {start=98;}
    if puppy_active_field(project) {start=start+86;}
    return w-513-start<path_width;
}
fn puppy_toolbar_breadcrumbs(project,rgb,w,x,right) {
    if right<=x {return x;}
    let heading=puppy_workspace_heading(project,0);
    let part=cad_project_part(project);
    let assembly=puppy_workspace_parent(project,part);
    let assembly_label="";let part_label="";
    let divider=" › ";let divider_width=puppy_toolbar_text_width(divider);
    let heading_width=puppy_toolbar_text_width(heading);
    let required=heading_width;let links=0;
    if assembly {
        assembly_label=m_cat("Assembly / ",puppy_design_item_name(assembly,"assembly"));
        required=required+puppy_toolbar_text_width(assembly_label)+divider_width;links=links+1;
    }
    if puppy_sketch && part {
        part_label=m_cat("Part / ",puppy_document_name(project));
        required=required+puppy_toolbar_text_width(part_label)+divider_width;links=links+1;
    }
    if assembly && required>right-x {
        required=required-puppy_toolbar_text_width(assembly_label)+puppy_toolbar_text_width("Assembly");
        assembly_label="Assembly";
    }
    if m_len(part_label) && required>right-x {
        required=required-puppy_toolbar_text_width(part_label)+puppy_toolbar_text_width("Part");
        part_label="Part";
    }
    if required>right-x {
        heading=puppy_workspace_heading(project,1);
        let compact_width=puppy_toolbar_text_width(heading);
        required=required-heading_width+compact_width;heading_width=compact_width;
    }
    let pad=0;if links {pad=puppy_limit((right-x-required-4)/(2*links),0,5);}
    if assembly {
        puppy_breadcrumb_assembly_left=x;
        x=puppy_bar_breadcrumb(rgb,w,x,right,assembly_label,pad);
        puppy_breadcrumb_assembly_right=x;
        puppy_bar_text(rgb,w,x,25,divider,right);x=puppy_limit(x+divider_width,x,right);
    }
    if m_len(part_label) {
        puppy_breadcrumb_part_left=x;
        x=puppy_bar_breadcrumb(rgb,w,x,right,part_label,pad);
        puppy_breadcrumb_part_right=x;
        puppy_bar_text(rgb,w,x,25,divider,right);x=puppy_limit(x+divider_width,x,right);
    }
    let end=x+puppy_limit(heading_width+4,0,right-x);
    puppy_bar_text(rgb,w,x,25,heading,end);
    return end;
}
fn puppy_toolbar_build(w) {
    puppy_breadcrumb_assembly_right=0;puppy_breadcrumb_part_right=0;
    if puppy_reader_report {
        let rgb=m_take(w*76*4);puppy_reader_bar(rgb,w);puppy_ui_upload(rgb,w,76);
        puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    let project=cad_project_open(puppy_history+24);
    let part=cad_project_item(project);let feature=puppy_feature(project);
    let rgb=m_take(w*76*4);puppy_bar_rect(rgb,w,0,0,w,76,30,43,47);
    puppy_bar_rect(rgb,w,0,75,w,1,73,96,95);
    if puppy_extrusion_open || puppy_revolve_open || puppy_shell_open {
        if puppy_number_text {puppy_number_bar(rgb,w,project);}
        else {
            if puppy_status {puppy_bar_text(rgb,w,12,25,puppy_status+24,w-8);}
            if w<800 {puppy_compact_bar(rgb,w,project,-1);}
            else {puppy_action_bar(puppy_feature_preview_actions(),rgb,w,-1);}
        }
        puppy_ui_upload(rgb,w,76);puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_plane_chooser {
        if puppy_status {puppy_bar_text(rgb,w,12,25,puppy_status+24,w-8);}
        puppy_action_bar(puppy_plane_actions(),rgb,w,-1);
        puppy_ui_upload(rgb,w,76);puppy_toolbar_width=w;puppy_toolbar_dirty=0;
        return 1;
    }
    if puppy_sheet_scene {
        let scene=j_parse(puppy_sheet_scene+24);
        let title=j_string(scene,"name");
        if puppy_status {title=puppy_status+24;}
        if puppy_sheet_entry {puppy_number_bar(rgb,w,project);}
        else {
            puppy_bar_text(rgb,w,12,25,title,w-10);
            puppy_action_bar(puppy_sheet_actions(),rgb,w,-1);
        }
        puppy_ui_upload(rgb,w,76);puppy_toolbar_width=w;puppy_toolbar_dirty=0;
        return 1;
    }
    if puppy_step_input || puppy_mating || puppy_construction_ids {
        if puppy_status {puppy_bar_text(rgb,w,12,25,puppy_status+24,w-12);}
        let rows=0;if puppy_construction_ids {rows=puppy_construction_actions();}else if puppy_mating {rows=puppy_mate_actions();}else {rows=puppy_step_actions(project);}
        puppy_action_bar(rows,rgb,w,-1);puppy_ui_upload(rgb,w,76);
        puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_history_rows {
        puppy_bar_text(rgb,w,12,25,"Feature history",135);
        if puppy_status {puppy_bar_text(rgb,w,142,25,puppy_status+24,w-12);}
        puppy_compact_bar(rgb,w,project,-1);puppy_ui_upload(rgb,w,76);
        puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_edge_kind {
        puppy_bar_text(rgb,w,12,25,puppy_edge_label(),112);
        if puppy_status {puppy_bar_text(rgb,w,120,25,puppy_status+24,w-12);}
        if w<800 {puppy_compact_bar(rgb,w,project,-1);}else {puppy_action_bar(puppy_edge_actions(),rgb,w,-1);}
        puppy_ui_upload(rgb,w,76);puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_loft_open {
        let sketch=puppy_draft_sketch(project);let name=j_string(sketch,"name");
        let label=m_cat(name,m_cat(" / Profile ",m_int(puppy_profile_index)));
        puppy_bar_text(rgb,w,12,25,label,w/2);
        if puppy_status {puppy_bar_text(rgb,w,w/2,25,puppy_status+24,w-10);}
        if w<800 {puppy_compact_bar(rgb,w,project,-1);}else {puppy_action_bar(puppy_loft_actions(),rgb,w,-1);}
        puppy_ui_upload(rgb,w,76);puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_number_text {
        puppy_number_bar(rgb,w,project);puppy_ui_upload(rgb,w,76);
        puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_patterns {
        let label=puppy_pattern_hint(project);if puppy_status {label=puppy_status+24;}puppy_bar_text(rgb,w,12,25,label,w-12);
        if w<800 {puppy_compact_bar(rgb,w,project,-1);}else {puppy_action_bar(puppy_pattern_actions(project),rgb,w,-1);}
        puppy_ui_upload(rgb,w,76);puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_line_report {
        let label=puppy_line_label();puppy_bar_text(rgb,w,12,25,label,w/2);
        let status_right=w-8;
        if puppy_direction_available() {status_right=puppy_direction_button_left(w)-8;puppy_direction_draw(rgb,w);}
        if puppy_status && !m_eq(label,puppy_status+24) {puppy_bar_text(rgb,w,w/2,25,puppy_status+24,status_right);}
        if w<800 {puppy_compact_bar(rgb,w,project,-1);}else {puppy_action_bar(puppy_line_actions(),rgb,w,-1);}
        puppy_ui_upload(rgb,w,76);puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    let compact=puppy_workspace_toolbar_compact(project,w);
    let title_x=12;
    if puppy_isolated {
        puppy_bar_rect(rgb,w,12,6,86,28,45,64,67);puppy_bar_text(rgb,w,20,25,"Show all",98);title_x=108;
    }
    if !puppy_sketch && !puppy_empty {
        puppy_bar_rect(rgb,w,title_x,6,76,28,45,64,67);
        puppy_bar_text(rgb,w,title_x+8,25,"Objects",title_x+76);
        title_x=title_x+86;
    }
    if !compact && puppy_draft {
        puppy_bar_rect(rgb,w,12,6,76,28,45,64,67);puppy_bar_text(rgb,w,20,25,"Revolve",88);title_x=98;
    }
    if !compact && puppy_active_field(project) {
        puppy_bar_rect(rgb,w,title_x,6,76,28,45,64,67);puppy_bar_text(rgb,w,title_x+8,25,"Set value",title_x+76);
        title_x=title_x+86;
    }
    let status_edge=w-335;
    if cad_project_is_studio(project) {status_edge=w-268;}
    if cad_project_part(project) {status_edge=w-513;}
    if compact {status_edge=w-8;}
    let title_end=puppy_toolbar_breadcrumbs(project,rgb,w,title_x,status_edge);
    if !cad_project_is_studio(project) && !compact {
        puppy_bar_rect(rgb,w,w-327,6,84,28,45,64,67);puppy_bar_text(rgb,w,w-319,25,"New part",w-243);
        puppy_bar_rect(rgb,w,w-235,6,118,28,45,64,67);puppy_bar_text(rgb,w,w-227,25,"New assembly",w-117);
        if puppy_draft || puppy_has_other_item(project) {
            let label="Next item";if puppy_draft {label="Done sketch";}
            puppy_bar_rect(rgb,w,w-109,6,97,28,45,64,67);
            puppy_bar_text(rgb,w,w-102,25,label,w-12);
        }
    }else if cad_project_is_studio(project) && !compact {
        puppy_bar_rect(rgb,w,w-260,6,112,28,45,64,67);puppy_bar_text(rgb,w,w-252,25,"Import STEP",w-148);
        let label="Perspective";if puppy_orthographic {label="Orthographic";}
        puppy_bar_rect(rgb,w,w-140,6,128,28,45,64,67);
        puppy_bar_text(rgb,w,w-132,25,label,w-12);
    }
    if compact {
        let info="";if puppy_status {info=puppy_status+24;}
        if !m_len(info) || m_eq(info,"Edited") {
            if cad_project_is_studio(project) {info=puppy_studio_label(project);}
            else if feature {info=m_cat(puppy_field(feature),m_cat(" ",m_cat(puppy_display_dimension(project,feature),puppy_property_unit(feature,puppy_property))));}
            else if m_eq(j_string(part,"type"),"assembly") {
                let data=j_get(part,"data");let servo=puppy_servo(data);if servo {info=puppy_servo_label(data,servo);}
            }
        }
        puppy_bar_text(rgb,w,title_end+4,25,info,w-8);
        puppy_compact_bar(rgb,w,project,-1);puppy_ui_upload(rgb,w,76);
        puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
    }
    if puppy_draft {
        let label="More tools";if puppy_curves_page==1 {label="Curves";}if puppy_curves_page==2 {label="Conics";}if puppy_curves_page==3 {label="Profiles";}
        if puppy_curves_page==4 {label="Edit";}
        puppy_bar_rect(rgb,w,w-420,6,85,28,45,64,67);puppy_bar_text(rgb,w,w-413,25,label,w-335);
    }
    if !puppy_sketch && !puppy_empty && cad_project_part(project) {
        puppy_bar_rect(rgb,w,w-420,6,85,28,45,64,67);puppy_bar_text(rgb,w,w-413,25,"Edges",w-335);
    }
    if cad_project_part(project) {
        puppy_bar_rect(rgb,w,w-505,6,76,28,45,64,67);
        puppy_bar_text(rgb,w,w-497,25,"Loft",w-429);
    }
    if puppy_status && status_edge-title_end>=72 {
        puppy_bar_text(rgb,w,title_end+8,25,puppy_status+24,status_edge);
    }
    puppy_bar_history(rgb,w,12,52,0);puppy_bar_history(rgb,w,70,52,1);
    puppy_bar_button(rgb,w,128,52,"Save");
    if cad_project_is_studio(project) {
        if puppy_studio_field(project) {
            puppy_bar_button(rgb,w,194,38,"-");puppy_bar_button(rgb,w,238,38,"+");puppy_bar_button(rgb,w,288,78,"Next field");
        }
        puppy_bar_button(rgb,w,376,68,"Export");let left=452;
        if puppy_pick_id && j_get(project,"assembly") {
            let pins=j_get(j_get(project,"assembly"),"fixed_instances");let label="Fix";if j_get(pins,puppy_pick_id+24) {label="Unfix";}
            puppy_bar_button(rgb,w,452,97,label);left=557;
        }
        puppy_bar_text(rgb,w,left,60,puppy_studio_label(project),w-140);
    }else if puppy_draft && puppy_revolve_axis {
        puppy_bar_button(rgb,w,194,76,"X axis");puppy_bar_button(rgb,w,278,76,"Y axis");puppy_bar_button(rgb,w,362,76,"Cancel");
    }else if puppy_draft && puppy_curves_page {puppy_curve_bar(project,rgb,w,-1);}
    else if puppy_draft {
        puppy_bar_button(rgb,w,194,86,"Rectangle");puppy_bar_button(rgb,w,288,67,"Circle");
        puppy_bar_button(rgb,w,560,60,"Line");
        if puppy_tool==2 {puppy_bar_rect(rgb,w,560,66,60,2,143,217,184);}
        else if puppy_tool==1 {puppy_bar_rect(rgb,w,288,66,67,2,143,217,184);}
        else if puppy_tool==0 {puppy_bar_rect(rgb,w,194,66,86,2,143,217,184);}
        puppy_bar_button(rgb,w,364,81,"Extrude");puppy_bar_button(rgb,w,452,100,m_cat("Profile ",m_int(puppy_profile_index)));
        let label_x=628;if puppy_line_button(project) {puppy_bar_button(rgb,w,628,90,"Dimensions");label_x=726;}
        if puppy_draft_on_face(project) {puppy_bar_button(rgb,w,628,85,puppy_operation(project));label_x=721;}
        if puppy_pending_face_index(project)>=0 {puppy_bar_button(rgb,w,721,70,"Solid");label_x=799;}
        if w>=920 {puppy_bar_button(rgb,w,label_x,80,"Patterns");label_x=label_x+88;}
        puppy_bar_text(rgb,w,label_x,60,j_string(puppy_draft_sketch(project),"name"),w-68);
    }else if m_eq(j_string(part,"type"),"assembly") {
        let data=j_get(part,"data");let servo=puppy_servo(data);
        if !puppy_empty {puppy_bar_button(rgb,w,288,78,"Solve");puppy_bar_button(rgb,w,376,68,"Export");}
        puppy_bar_button(rgb,w,452,69,"Insert");
        let choice_width=puppy_limit(w-747,0,170);if servo {choice_width=puppy_limit(choice_width,0,96);}
        puppy_bar_button(rgb,w,529,choice_width,m_cat("Part: ",j_string(puppy_insert_part_choice(project),"name")));
        if !puppy_empty {puppy_bar_button(rgb,w,w-211,71,"Mate");}
        if servo {
            puppy_bar_button(rgb,w,194,38,"-");puppy_bar_button(rgb,w,238,38,"+");
            puppy_bar_text(rgb,w,537+choice_width,60,puppy_servo_label(data,servo),w-215);
        }
    }else if feature {
        puppy_bar_button(rgb,w,194,38,"-");puppy_bar_button(rgb,w,238,38,"+");
        let label=m_cat(j_string(feature,"name"),m_cat(" / ",m_cat(puppy_field(feature),m_cat("   ",m_cat(puppy_display_dimension(project,feature),puppy_property_unit(feature,puppy_property))))));
        puppy_bar_button(rgb,w,376,68,"Export");
        let label_x=656;
        if w>=900 && puppy_line_button(project) {puppy_bar_button(rgb,w,656,90,"Dimensions");label_x=754;}
        puppy_bar_text(rgb,w,label_x,60,label,w-140);
    }
    if !puppy_draft && cad_project_part(project) {
        puppy_bar_sketch(rgb,w,452,108,"New sketch");
        if puppy_picking_face {puppy_bar_rect(rgb,w,452,66,108,2,143,217,184);}

        if puppy_draft_sketch(project) {
            if puppy_sketch {puppy_bar_button(rgb,w,288,84,"Part");}
            else {puppy_bar_sketch(rgb,w,288,84,"Sketch");}
        }
        if feature {puppy_bar_button(rgb,w,568,80,"History");}
    }
    if !puppy_draft && !puppy_empty {puppy_bar_button(rgb,w,w-132,64,"STEP");}
    if puppy_draft || puppy_camera_available() {puppy_bar_button(rgb,w,w-60,48,"Fit");}
    puppy_ui_upload(rgb,w,76);
    puppy_toolbar_width=w;puppy_toolbar_dirty=0;return 1;
}
fn puppy_breadcrumb_target(x,y) {
    if y<6 || y>=34 {return 0;}
    if x>=puppy_breadcrumb_assembly_left && x<puppy_breadcrumb_assembly_right {return 1;}
    if puppy_sketch && x>=puppy_breadcrumb_part_left && x<puppy_breadcrumb_part_right {return 2;}
    return 0;
}
fn puppy_breadcrumb_click(project,x,y) {
    let target=puppy_breadcrumb_target(x,y);
    if target==1 {return puppy_workspace_return_to_assembly(project);}
    if target==2 {return puppy_workspace_return_to_part(project);}
    return 0;
}
fn puppy_ui_upload(rgb,w,h) {
    let args=m_take(88);gui__zero(args,88);
    store64(args,0x0de1);store64(args+16,0x8058);store64(args+24,w);store64(args+32,h);
    store64(args+48,0x1908);store64(args+56,0x1401);store64(args+64,rgb);
    ffi_call_words(gui_gl_cached_proc(load64(puppy_device),"glTexImage2D"),args);
    render_gl(puppy_device,"glTexParameteri",0x0de1,0x2801,0x2600,0,0,0);
    render_gl(puppy_device,"glTexParameteri",0x0de1,0x2800,0x2600,0,0,0);
    return 1;
}
fn puppy_toolbar_draw(w,h) {
    let window=load64(puppy_device);let logical=gui_width(window);
    if logical<=0 || logical>16384 {return 0;}
    render_gl(puppy_device,"glUseProgram",puppy_toolbar_program,0,0,0,0,0);
    render_gl(puppy_device,"glActiveTexture",0x84c0,0,0,0,0,0);
    render_gl(puppy_device,"glBindTexture",0x0de1,puppy_toolbar_texture,0,0,0,0);
    if puppy_toolbar_width!=logical || puppy_toolbar_dirty {puppy_toolbar_build(logical);}
    let height=76*h/gui_height(window);
    render_gl(puppy_device,"glViewport",0,h-height,w,height,0,0);
    render_gl(puppy_device,"glDisable",0x0b71,0,0,0,0,0);
    render_gl(puppy_device,"glDisable",0x0b44,0,0,0,0,0);
    return render_draw(puppy_device,puppy_toolbar_vao,4,0,3);
}

// A compact window pages through the same editor commands. Drawing and input
// use one ordered list so a clipped or invisible button cannot receive a click.
fn puppy_compact_button(rows,label,key,mods) {return cg_push(rows,cg_v(label,key,mods));}
fn puppy_curve_buttons(rows) {
    puppy_compact_button(rows,"3-point arc",4,0);puppy_compact_button(rows,"Fit spline",22,0);
    puppy_compact_button(rows,"Control spline",5,0);puppy_compact_button(rows,"Periodic spline",14,0);
    if puppy_tool>=4 && puppy_tool<=6 {puppy_compact_button(rows,"Finish",40,0);}
    puppy_compact_button(rows,"Point",55,0);puppy_compact_button(rows,"Datum",55,3);puppy_compact_button(rows,"Offset",61,0);
    puppy_compact_button(rows,"Tangent arc",13,3);puppy_compact_button(rows,"Smooth",13,0);
}
fn puppy_profile_buttons(rows) {
    puppy_compact_button(rows,"Diameter circle",7,0);
    puppy_compact_button(rows,"Center rectangle",21,3);
    puppy_compact_button(rows,"3-pt rectangle",23,0);
    puppy_compact_button(rows,"3-pt centered",23,3);
    puppy_compact_button(rows,"Polygon",10,3);
    puppy_compact_button(rows,"Slot",22,3);
}
fn puppy_curve_bar(project,rgb,w,x) {
    if puppy_curves_page==4 {
        if x>=194 && x<279 {return puppy_command(20,0);}
        if x>=287 && x<372 {return puppy_command(20,3);}
        if rgb {
            puppy_bar_button(rgb,w,194,85,"Trim");puppy_bar_button(rgb,w,287,85,"Extend");
            if puppy_tool==-8 {puppy_bar_rect(rgb,w,194,66,85,2,143,217,184);}
            if puppy_tool==-9 {puppy_bar_rect(rgb,w,287,66,85,2,143,217,184);}
        }
        return 1;
    }
    if puppy_curves_page==3 {
        let rows=cg_array();puppy_profile_buttons(rows);
        let left=194;let width=puppy_limit((w-68-left-8*(cg_count(rows)-1))/cg_count(rows),1,144);let i=0;
        while i<cg_count(rows) {
            let row=cg_at(rows,i);
            if x>=left && x<left+width {return puppy_command(load64(row+8),load64(row+16));}
            if rgb {
                let label=load64(row);
                if width<110 {
                    if i==0 {label="Dia. circle";}
                    if i==1 {label="Center rect.";}
                    if i==2 {label="3-pt rect.";}
                    if i==3 {label="3-pt center";}
                }
                puppy_bar_button(rgb,w,left,width,label);
                if puppy_tool==i+9 {puppy_bar_rect(rgb,w,left,66,width,2,143,217,184);}
            }
            left=left+width+8;i=i+1;
        }
        return 1;
    }

    if puppy_curves_page==2 {
        if x>=194 && x<280 {return puppy_command(8,3);}if x>=288 && x<416 {return puppy_command(6,3);}if x>=424 && x<534 {return puppy_command(27,3);}
        if x>=542 && x<664 {return puppy_command(26,0);}
        if puppy_draft_on_face(project) {
            let width=puppy_limit(w-740,0,100);
            let pick_width=puppy_limit(w-848,0,100);
            if x>=672 && x<672+width {return puppy_command(24,3);}
            if pick_width>=80 && x>=780 && x<780+pick_width {return puppy_command(24,0);}
            if rgb {
                let label="Use edges";if width<80 {label="Edges";}
                puppy_bar_button(rgb,w,672,width,label);
                if pick_width>=80 {puppy_bar_button(rgb,w,780,pick_width,"Pick edge");}
            }
        }
        if rgb {
            puppy_bar_button(rgb,w,194,86,"Ellipse");puppy_bar_button(rgb,w,288,128,"3-point circle");puppy_bar_button(rgb,w,424,110,"Split");puppy_bar_button(rgb,w,542,122,"Construction");
            if puppy_tool==7 {puppy_bar_rect(rgb,w,194,66,86,2,143,217,184);}
            if puppy_tool==-6 {puppy_bar_rect(rgb,w,424,66,110,2,143,217,184);}
            if puppy_tool==8 {puppy_bar_rect(rgb,w,288,66,128,2,143,217,184);}
        }return 1;
    }
    let rows=cg_array();puppy_curve_buttons(rows);let widths=cg_v(94,88,112);let left=194;let i=0;
    let total=94+88+112+115+54+66+66+92+68;if puppy_tool>=4 && puppy_tool<=6 {total=total+72;}
    let space=puppy_limit(w-68-left-8*(cg_count(rows)-1),1,total);
    while i<cg_count(rows) {
        let entry=cg_at(rows,i);let key=load64(entry+8);let mods=load64(entry+16);let label=load64(entry);
        let width=115;if i<3 {width=load64(widths+i*8);}if key==40 {width=72;}
        let active=i<4 && puppy_tool==i+3;
        if key==55 {width=54;active=puppy_tool==-1;if mods {width=66;active=puppy_tool==-2;}}
        if key==61 {width=66;active=puppy_tool==-3;}
        if key==13 {width=68;active=puppy_tool==-5;if mods {width=92;active=puppy_tool==-4;if space<total {label="Tangent";}}}
        width=width*space/total;
        if space<total && i==0 {label="Arc";}if space<total && i==2 {label="Control";}if space<total && i==3 {label="Periodic";}
        if x>=left && x<left+width {return puppy_command(key,mods);}
        if rgb {puppy_bar_button(rgb,w,left,width,label);if active {puppy_bar_rect(rgb,w,left,66,width,2,143,217,184);}}
        left=left+width+8;i=i+1;
    }
    return 1;
}
fn puppy_compact_actions(project) {
    if puppy_extrusion_open || puppy_revolve_open || puppy_shell_open {return puppy_feature_preview_actions();}
    if puppy_patterns {return puppy_pattern_actions(project);}
    if puppy_line_report {return puppy_line_actions();}
    if puppy_history_rows {return puppy_history_actions(project);}
    if puppy_edge_kind {return puppy_edge_actions();}
    if puppy_loft_open {return puppy_loft_actions();}
    let rows=cg_array();let studio=cad_project_is_studio(project);
    let solid=!puppy_sketch && !puppy_empty;let assembly=m_eq(j_string(cad_project_item(project),"type"),"assembly");
    puppy_compact_button(rows,"Undo",29,192);puppy_compact_button(rows,"Redo",28,192);
    puppy_compact_button(rows,"Save",22,192);
    if studio || (!puppy_draft && !puppy_empty) {
        let editable=0;if studio {editable=puppy_studio_field(project);}
        else if assembly {editable=puppy_servo(j_get(cad_project_item(project),"data"));}
        else {editable=puppy_feature(project);}
        if editable {
            puppy_compact_button(rows,"−",45,0);puppy_compact_button(rows,"+",46,0);
            puppy_compact_button(rows,"Next field",43,0);
        }
        puppy_compact_button(rows,"Export STL",8,192);puppy_compact_button(rows,"Export 3MF",8,960);
        puppy_compact_button(rows,"Export STEP",8,195);
        if studio {puppy_compact_button(rows,"Import STEP",12,192);}
    }
    if studio {
        if puppy_pick_id && j_get(project,"assembly") {
            let label="Fix";if j_get(j_get(j_get(project,"assembly"),"fixed_instances"),puppy_pick_id+24) {label="Unfix";}
            puppy_compact_button(rows,label,19,0);
        }
    }else {
        if puppy_draft && puppy_revolve_axis {
            puppy_compact_button(rows,"X axis",27,0);puppy_compact_button(rows,"Y axis",28,0);
            puppy_compact_button(rows,"Cancel",37,0);
        }else if puppy_draft {
            puppy_compact_button(rows,"Done sketch",69,0);
            puppy_compact_button(rows,"Rectangle",21,0);puppy_compact_button(rows,"Circle",6,0);
            puppy_compact_button(rows,"Line",15,0);puppy_compact_button(rows,"Extrude",27,0);
            puppy_compact_button(rows,"Revolve",37,0);puppy_curve_buttons(rows);puppy_compact_button(rows,"Ellipse",8,3);puppy_compact_button(rows,"3-point circle",6,3);puppy_compact_button(rows,"Split",27,3);puppy_compact_button(rows,"Construction",26,0);puppy_compact_button(rows,"Fit sketch",74,0);
            puppy_profile_buttons(rows);
            puppy_compact_button(rows,"Next profile",19,0);puppy_compact_button(rows,"Patterns",62,0);
            puppy_compact_button(rows,"Trim",20,0);puppy_compact_button(rows,"Extend",20,3);
            if puppy_draft_on_face(project) {
                puppy_compact_button(rows,puppy_operation(project),18,0);
                puppy_compact_button(rows,"Use edges",24,3);puppy_compact_button(rows,"Pick edge",24,0);
            }
            if puppy_pending_face_index(project)>=0 {puppy_compact_button(rows,"Solid",25,0);}
        }else if assembly {
            if !puppy_empty {puppy_compact_button(rows,"Solve",25,0);puppy_compact_button(rows,"Mate",13,0);}
            puppy_compact_button(rows,"Choose part",19,0);puppy_compact_button(rows,"Insert",12,0);
        }else {
            if puppy_draft_sketch(project) {
                let label="Edit sketch";if puppy_sketch {label="Part";}
                puppy_compact_button(rows,label,69,0);
            }
            puppy_compact_button(rows,"New sketch",22,3);
            if m_eq(j_string(puppy_feature(project),"type"),"extrude") {puppy_compact_button(rows,"Extrusion options",61,0);}
        }
        if !assembly {
            puppy_compact_button(rows,"Loft",38,0);
            if solid {puppy_compact_button(rows,"Edges",11,0);}
        }
        puppy_compact_button(rows,"New part",17,3);puppy_compact_button(rows,"New assembly",4,3);
        if puppy_has_other_item(project) {puppy_compact_button(rows,"Next item",63,0);}
    }
    if puppy_camera_available() {
        puppy_compact_button(rows,"Fit",74,0);
        let projection="Orthographic";if puppy_orthographic {projection="Perspective";}
        puppy_compact_button(rows,projection,18,0);
    }
    if cad_project_part(project) {puppy_compact_button(rows,"History",64,0);puppy_compact_button(rows,"Dimensions",65,0);}
    if puppy_active_field(project) {puppy_compact_button(rows,"Set value",59,0);}
    return rows;
}
fn puppy_compact_bar(rgb,w,project,click) {
    cg_begin();let rows=puppy_compact_actions(project);let slots=puppy_limit((w-72)/104,1,8);
    let cell=puppy_limit((w-72)/slots,1,300);let pages=(cg_count(rows)+slots-1)/slots;
    puppy_bar_page=puppy_limit(puppy_bar_page,0,pages-1);
    if click>=0 {
        if click>=6 && click<34 {puppy_bar_page=(puppy_bar_page+pages-1)%pages;}
        else if click>=w-34 && click<w-6 {puppy_bar_page=(puppy_bar_page+1)%pages;}
        else if click>=36 && click<36+slots*cell {
            let entry=cg_at(rows,puppy_bar_page*slots+(click-36)/cell);
            if entry {return puppy_command(load64(entry+8),load64(entry+16));}
        }
        puppy_toolbar_dirty=1;return 1;
    }
    puppy_bar_button(rgb,w,6,28,"‹");puppy_bar_button(rgb,w,w-34,28,"›");
    let i=0;while i<slots {
        let entry=cg_at(rows,puppy_bar_page*slots+i);
        if entry {
            let x=36+i*cell;let width=cell-3;let label=load64(entry);
            if m_eq(label,"Undo") || m_eq(label,"Redo") {puppy_bar_history(rgb,w,x,width,m_eq(label,"Redo"));}
            else if m_eq(label,"Edit sketch") {puppy_bar_sketch(rgb,w,x,width,"Sketch");}
            else if m_eq(label,"New sketch") {puppy_bar_sketch(rgb,w,x,width,label);}
            else {puppy_bar_button(rgb,w,x,width,label);}
        }
        i=i+1;
    }
    return 1;
}
