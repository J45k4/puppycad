import "design-tree.flex";
// The panel indexes evaluated occurrences by their full ID. Identically named
// copies can be selected independently without modifying the document.
global puppy_catalog=0;
global puppy_placement_open=0;
global puppy_placement_slot=-1;
global puppy_tree_open=0;
global puppy_tree_texture=0;
global puppy_tree_dirty=1;
global puppy_tree_width=0;
global puppy_tree_height=0;
global puppy_tree_scroll=0;
global puppy_tree_folded=0;
global puppy_folded_paths=0;
global puppy_search=0;
global puppy_search_active=0;
global puppy_definition=0;
global puppy_copy_index=0;
global puppy_copy_focus=0;
global puppy_definition_click_time=0;
global puppy_pointer_x=0;
global puppy_pointer_y=0;

fn puppy_objects_button(x) {
    let left=12;if puppy_isolated {left=108;}return x>=left && x<left+76;
}
fn puppy_search_focus(active) {
    if active!=puppy_search_active {
        puppy_search_active=active;gui_text_focus(load64(puppy_device),active);puppy_tree_dirty=1;
    }
    return 1;
}
fn puppy_objects_toggle() {
    puppy_design_focus=0;puppy_search_focus(0);puppy_query_set("");puppy_tree_open=!puppy_tree_open;
    if !puppy_tree_open {puppy_search_focus(0);}
    puppy_tree_dirty=1;return 1;
}
fn puppy_objects_width() {
    let w=gui_width(load64(puppy_device));return puppy_limit(w*2/3,1,288);
}
fn puppy_select_entry(entry) {
    puppy_clear_pick();puppy_pick_id=puppy_snapshot(j_string(entry,"id"));
    puppy_first=j_integer(entry,"first");puppy_last=puppy_first+j_integer(entry,"count");
    return 1;
}
fn puppy_fold_ascii(c) {if c>=65 && c<=90 {return c+32;}return c;}
fn puppy_matches(text,query) {
    if !query || !load8(query) {return 1;}
    let i=0;while load8(text+i) {
        let j=0;while load8(query+j) && puppy_fold_ascii(load8(text+i+j))==puppy_fold_ascii(load8(query+j)) {j=j+1;}
        if !load8(query+j) {return 1;}i=i+1;
    }
    return 0;
}
fn puppy_query() {if puppy_search {return puppy_search+24;}return "";}
fn puppy_query_set(text) {
    let next=0;if m_len(text) {next=puppy_snapshot(text);}
    if puppy_search {puppy_drop(puppy_search);}puppy_search=next;
    puppy_tree_scroll=0;puppy_design_scroll=0;puppy_tree_dirty=1;return 1;
}
fn puppy_panel_group(rows,children,bit,label) {
    if !cg_count(children) {return 0;}
    cg_push(rows,cg_v(0,label,bit));
    if !(puppy_tree_folded&bit) || load8(puppy_query()) {
        let i=0;while i<cg_count(children) {cg_push(rows,cg_at(children,i));i=i+1;}
    }
    return 1;
}
fn puppy_panel_field(fields,index,property,label,id,query) {
    if puppy_matches(label,query) || puppy_matches(id,query) {cg_push(fields,cg_v(2,label,cg_v(index,property,0)));}
    return 1;
}
fn puppy_branch_states() {
    if puppy_folded_paths {return j_parse(puppy_folded_paths+24);}return j_object();
}
fn puppy_branch_depth(id) {
    let i=0;let depth=0;while load8(id+i) {if load8(id+i)==47 {depth=depth+1;}i=i+1;}return depth;
}
fn puppy_add_parents(rows,seen,folded,id,query) {
    let i=0;let start=0;
    while load8(id+i) {
        if load8(id+i)==47 {
            let path=m_slice(id,i);let closed=j_get(folded,path) && !load8(query);
            if !j_get(seen,path) {
                let sign="− ";if closed {sign="+ ";}
                cg_push(rows,cg_v(3,m_cat(sign,m_slice(id+start,i-start)),path));j_add(seen,path,j_number(1));
            }
            if closed {return 0;}start=i+1;
        }
        i=i+1;
    }
    return 1;
}
fn puppy_toggle_branch(path) {
    let folded=puppy_branch_states();
    if !pcs_remove(folded,path) {j_add(folded,path,j_number(1));}
    if puppy_folded_paths {puppy_drop(puppy_folded_paths);}
    puppy_folded_paths=puppy_snapshot(j_encode(folded));return 1;
}
fn puppy_panel_rows(project,parts) {
    cg_begin();let rows=cg_array();let query=puppy_query();let bodies=cg_array();let part=pc_first(parts);
    let nested=cad_project_is_studio(project) && j_get(project,"assembly_definitions");
    let seen=j_object();let closed=puppy_branch_states();
    while part {
        let name=j_string(part,"name");let id=j_string(part,"id");
        if (puppy_matches(name,query) || puppy_matches(id,query)) && (!nested || puppy_add_parents(bodies,seen,closed,id,query)) {
            if !m_len(name) {name=id;}cg_push(bodies,cg_v(1,name,part));
        }
        part=load64(part+24);
    }
    let group="Bodies";let item=cad_project_item(project);
    if j_get(project,"assembly") || m_eq(j_string(item,"type"),"assembly") {group="Occurrences";}
    puppy_panel_group(rows,bodies,1,group);
    if puppy_pick_id && m_eq(j_string(item,"type"),"assembly") {
        let data=j_get(item,"data");let mates=cg_array();let mate=pc_first(j_get(data,"mates"));
        while mate {
            if pm_touches(mate,puppy_pick_id+24) {
                let other=puppy_other_instance(data,mate,puppy_pick_id+24);
                let label=m_cat(puppy_mate_name(mate),m_cat(" · ",other));
                if puppy_matches(label,query) || puppy_matches(j_string(mate,"id"),query) || puppy_matches(j_string(mate,"type"),query) {
                    cg_push(mates,cg_v(5,label,mate));
                }
            }mate=load64(mate+24);
        }
        puppy_panel_group(rows,mates,8,"Mates");
    }
    let definitions=0;
    if j_get(project,"assembly") {definitions=j_get(project,"parts");}
    else if m_eq(j_string(item,"type"),"assembly") {definitions=j_get(project,"items");}
    let definition=pc_first(definitions);let designs=cg_array();
    while definition {
        let id=j_string(definition,"id");let name=j_string(definition,"name");if !m_len(name) {name=id;}
        if (!j_get(project,"items") || m_eq(j_string(definition,"type"),"part")) && (puppy_matches(name,query) || puppy_matches(id,query)) {
            cg_push(designs,cg_v(4,name,definition));
        }
        definition=load64(definition+24);
    }
    puppy_panel_group(rows,designs,4,"Part definitions");
    let fields=cg_array();let index=0;
    if cad_project_is_studio(project) {
        let editable=cad_studio_properties(project);
        while index<cg_count(editable) {
            let f=cg_at(editable,index);let value=0;
            if load64(f+16) {value=cg_number(load64(f+16));}
            let label=m_cat(load64(f+24),m_cat("   ",m_cat(puppy_display_number(value),m_cat(" ",load64(f+32)))));
            puppy_panel_field(fields,index,0,label,j_string(load64(f),"id"),query);index=index+1;
        }
    }else if m_eq(j_string(item,"type"),"assembly") {
        let data=j_get(item,"data");let servo=pc_first(j_get(data,"actuators"));
        while servo {
            if m_eq(j_string(servo,"type"),"servo") && j_get(servo,"homeDeg") {
                puppy_panel_field(fields,index,0,puppy_servo_label(data,servo),j_string(servo,"id"),query);index=index+1;
            }
            servo=load64(servo+24);
        }
    }else {
        let bar=j_get(j_get(j_get(item,"data"),"tree"),"rollbackIndex");let position=0;
        let feature=pc_first(pc_nodes(item));while feature && (!bar || position<j_value(bar)) {
            if pc_model_feature(feature) {position=position+1;}
            if puppy_driving_feature(feature) {
                let rectangle=puppy_rectangle(project,feature);
                let count=puppy_property_count(feature,rectangle);let property=0;
                while property<count {
                    let name=puppy_property_name(feature,property);
                    let value=puppy_feature_measure(feature,property,rectangle);
                    let label=m_cat(j_string(feature,"name"),m_cat(" / ",m_cat(name,m_cat("   ",m_cat(puppy_display_number(value),puppy_property_unit(feature,property))))));
                    puppy_panel_field(fields,index,property,label,j_string(feature,"id"),query);
                    property=property+1;
                }
                index=index+1;
            }
            feature=load64(feature+24);
        }
    }
    puppy_panel_group(rows,fields,2,"Driving fields");return rows;
}
// The same row height and limits drive the drawing, scrolling and hit targets.
fn puppy_panel_capacity(h) {return puppy_limit((h-48-118)/28,0,200);}
fn puppy_panel_offset(rows,h) {
    let last=cg_count(rows)-puppy_panel_capacity(h);if last<0 {last=0;}
    puppy_tree_scroll=puppy_limit(puppy_tree_scroll,0,last);return puppy_tree_scroll;
}
fn puppy_part_copies(parts) {
    let copies=cg_array();let entry=pc_first(parts);
    while entry {
        if m_eq(j_string(entry,"part"),puppy_definition+24) {cg_push(copies,entry);}
        entry=load64(entry+24);
    }
    return copies;
}
fn puppy_copy_choose(project,copies,index) {
    let count=cg_count(copies);puppy_copy_focus=1;puppy_tree_dirty=1;
    if !count {return puppy_title(project,"No placed occurrences");}
    puppy_copy_index=(index+count)%count;
    return puppy_title(project,m_cat("Occurrence ",j_string(cg_at(copies,puppy_copy_index),"id")));
}
fn puppy_copy_isolate(project,copies) {
    let entry=cg_at(copies,puppy_copy_index);if !entry || puppy_body_count<2 {return 1;}
    puppy_select_entry(entry);return puppy_isolate(project,j_string(entry,"id"));
}
fn puppy_definition_inspector(rgb,w,h,parts) {
    let copies=puppy_part_copies(parts);let count=cg_count(copies);let y=h-115;
    puppy_ui_rect(rgb,w,h,0,y,w,1,72,91,92);
    puppy_ui_text(rgb,w,h,12,y+23,m_cat("Part  ",puppy_definition+24),w-6);
    if !count {puppy_ui_text(rgb,w,h,12,y+51,"No placed occurrences",w-6);return 1;}
    let entry=cg_at(copies,puppy_copy_index);
    puppy_ui_text(rgb,w,h,12,y+45,j_string(entry,"name"),w-6);
    puppy_ui_text(rgb,w,h,12,y+66,j_string(entry,"id"),w-6);
    if puppy_body_count>1 {
        puppy_ui_rect(rgb,w,h,10,h-37,84,28,50,73,75);puppy_ui_text(rgb,w,h,19,h-18,"Isolate",92);
    }
    if count>1 {
        puppy_ui_rect(rgb,w,h,102,h-37,32,28,50,73,75);puppy_ui_text(rgb,w,h,113,h-18,"‹",134);
        puppy_ui_rect(rgb,w,h,144,h-37,32,28,50,73,75);puppy_ui_text(rgb,w,h,154,h-18,"›",176);
        puppy_ui_text(rgb,w,h,184,h-18,m_cat(m_int(puppy_copy_index+1),m_cat(" / ",m_int(count))),w-6);
    }
    return 1;
}
fn puppy_mate_inspector(rgb,w,h,data,mate) {
    let y=h-115;puppy_ui_rect(rgb,w,h,0,y,w,1,72,91,92);
    puppy_ui_text(rgb,w,h,12,y+21,m_cat(puppy_mate_name(mate),m_cat(" · ",j_string(mate,"type"))),w-8);
    let a=j_string(j_get(mate,"a"),"connectorId");let b=j_string(j_get(mate,"b"),"connectorId");
    puppy_ui_text(rgb,w,h,12,y+44,m_cat(a,m_cat(" → ",b)),w-8);
    puppy_ui_text(rgb,w,h,12,y+66,j_string(mate,"id"),w-8);
    puppy_ui_rect(rgb,w,h,10,h-37,100,28,50,73,75);puppy_ui_text(rgb,w,h,18,h-18,"Remove mate",110);
    if m_eq(j_string(mate,"type"),"revolute") {
        let servo=pc_mate_servo(data,j_string(mate,"id"));let label="Set angle";
        if servo {label=m_cat(puppy_display_number(pc_pose_number(j_get(servo,"homeDeg")))," deg");}
        puppy_ui_rect(rgb,w,h,118,h-37,puppy_limit(w-128,0,128),28,50,73,75);puppy_ui_text(rgb,w,h,126,h-18,label,w-10);
    }return 1;
}
// The placement grid uses the same coordinates for painting and hit testing.
fn puppy_placement_group(slot) {
    if slot<3 {return "translation";}if slot<6 {return "rotation";}return "scale";
}
fn puppy_placement_axis(slot) {
    if slot%3==0 {return "x";}if slot%3==1 {return "y";}return "z";
}
fn puppy_placement_label(slot) {
    if slot<3 {return "Move (mm)";}if slot<6 {return "Rotate (°)";}return "Scale";
}
fn puppy_placement_cell(transform,slot) {
    return pc_pose_number(j_get(j_get(transform,puppy_placement_group(slot)),puppy_placement_axis(slot)));
}
fn puppy_placement_column(w) {return (w-94)/3;}
fn puppy_placement_grid(rgb,w,h,project) {
    let transform=cad_project_instance_transform(project,puppy_pick_id+24);let top=h-115;
    puppy_ui_rect(rgb,w,h,0,top,w,1,72,91,92);
    puppy_ui_text(rgb,w,h,10,top+20,m_cat("Placement · ",puppy_pick_id+24),w-58);
    puppy_ui_text(rgb,w,h,w-52,top+20,"Back",w-6);
    if !transform {puppy_ui_text(rgb,w,h,10,top+50,cg_error,w-6);return 1;}
    let width=puppy_placement_column(w);let slot=0;
    while slot<9 {
        let left=88+slot%3*width;let y=top+26+slot/3*29;
        if slot%3==0 {puppy_ui_text(rgb,w,h,8,y+17,puppy_placement_label(slot),86);}
        puppy_ui_rect(rgb,w,h,left,y,width-3,24,50,73,75);
        let label=m_cat(puppy_placement_axis(slot),m_cat(" ",puppy_display_number(puppy_placement_cell(transform,slot))));
        puppy_ui_text(rgb,w,h,left+4,y+17,label,left+width-4);slot=slot+1;
    }return 1;
}
fn puppy_placement_click(project,x,y,w) {
    if y<26 {
        if x>=w-58 {puppy_placement_open=0;puppy_tree_dirty=1;}return 1;
    }
    let width=puppy_placement_column(w);if width<4 || x<88 || x>=88+3*width || (x-88)%width>=width-3 {return 1;}
    let row=(y-26)/29;if row>=3 || (y-26)%29>=24 {return 1;}
    puppy_placement_slot=row*3+(x-88)/width;let field=puppy_active_field(project);
    if !field {puppy_placement_slot=-1;return puppy_title(project,cg_error);}return puppy_number_open(project,field);
}
fn puppy_panel_inspector(rgb,w,h,part,project) {
    let item=cad_project_item(project);
    if !part {return 1;}
    if puppy_selected_mate {
        let data=j_get(item,"data");let mate=pc_find(j_get(data,"mates"),puppy_selected_mate+24);
        if mate {return puppy_mate_inspector(rgb,w,h,data,mate);}
    }
    if puppy_placement_open {return puppy_placement_grid(rgb,w,h,project);}
    let y=h-115;puppy_ui_rect(rgb,w,h,0,y,w,1,72,91,92);let end=w-8;
    if m_eq(j_string(item,"type"),"assembly") {
        end=w-100;puppy_ui_rect(rgb,w,h,end,y+4,90,26,50,73,75);puppy_ui_text(rgb,w,h,end+8,y+22,"Placement",w-10);
    }
    puppy_ui_text(rgb,w,h,12,y+22,j_string(part,"name"),end);
    puppy_ui_text(rgb,w,h,12,y+42,j_string(part,"id"),w-8);
    let bounds=pc_first(j_get(part,"bounds"));let size=cg_sub(cg_vector(load64(bounds+24),0),cg_vector(bounds,0));
    let dimensions=m_cat(puppy_display_number(load64(size)),m_cat(" × ",puppy_display_number(load64(size+8))));
    puppy_ui_text(rgb,w,h,12,y+65,m_cat(dimensions,m_cat(" × ",m_cat(puppy_display_number(load64(size+16))," mm"))),w-8);
    if puppy_body_count>1 {
        let label="Isolate";if puppy_isolated {label="Show all";}
        puppy_ui_rect(rgb,w,h,10,h-37,94,28,50,73,75);puppy_ui_text(rgb,w,h,18,h-18,label,104);
    }
    if m_eq(j_string(item,"type"),"assembly") {
        puppy_ui_rect(rgb,w,h,110,h-37,puppy_limit(w-120,0,146),28,50,73,75);puppy_ui_text(rgb,w,h,118,h-18,"Remove instance",w-10);
    }return 1;
}
fn puppy_panel_build(w,h) {
    let project=cad_project_open(puppy_history+24);let parts=j_parse(puppy_catalog+24);
    let rows=puppy_panel_rows(project,parts);let first=puppy_panel_offset(rows,h);let query=puppy_query();
    let rgb=m_take(w*h*4);puppy_ui_rect(rgb,w,h,0,0,w,h,32,45,49);
    puppy_ui_rect(rgb,w,h,w-1,0,1,h,74,96,95);
    puppy_ui_rect(rgb,w,h,10,8,w-20,31,20,30,34);
    if puppy_search_active {puppy_ui_rect(rgb,w,h,10,37,w-20,2,143,217,184);}
    let text=query;if !load8(query) {text="Search names or IDs";}puppy_ui_text(rgb,w,h,18,29,text,w-42);
    if load8(query) {puppy_ui_text(rgb,w,h,w-34,29,"×",w-12);}
    let j=0;while j<puppy_panel_capacity(h) && first+j<cg_count(rows) {
        let row=cg_at(rows,first+j);let kind=load64(row);let y=48+j*28;let label=load64(row+8);
        if !kind {
            let sign="−";if !load8(query) && (puppy_tree_folded&load64(row+16)) {sign="+";}
            puppy_ui_text(rgb,w,h,12,y+19,m_cat(sign,m_cat("  ",label)),w-8);
        }else {
            let selected=kind==2 && !puppy_definition && !puppy_pick_id && puppy_selected==load64(load64(row+16)) && puppy_property==load64(load64(row+16)+8);
            if kind==1 && puppy_pick_id {selected=m_eq(j_string(load64(row+16),"id"),puppy_pick_id+24);}
            if kind==5 && puppy_selected_mate {selected=m_eq(j_string(load64(row+16),"id"),puppy_selected_mate+24);}
            if kind==4 && puppy_definition {selected=m_eq(j_string(load64(row+16),"id"),puppy_definition+24);}
            if selected {puppy_ui_rect(rgb,w,h,8,y,w-16,26,54,77,75);}
            let left=30;
            if kind==1 {left=left+12*puppy_limit(puppy_branch_depth(j_string(load64(row+16),"id")),0,4);}
            if kind==3 {left=left+12*puppy_branch_depth(load64(row+16));}
            puppy_ui_text(rgb,w,h,left,y+18,label,w-10);
        }
        j=j+1;
    }
    if !cg_count(rows) {puppy_ui_text(rgb,w,h,12,68,"No matches",w-8);}
    if puppy_definition && h>=175 {puppy_definition_inspector(rgb,w,h,parts);}
    else if puppy_pick_id && h>=175 {puppy_panel_inspector(rgb,w,h,pc_find(parts,puppy_pick_id+24),project);}
    puppy_ui_upload(rgb,w,h);puppy_tree_width=w;puppy_tree_height=h;puppy_tree_dirty=0;return 1;
}
fn puppy_objects_draw(w,h) {
    let design=puppy_design_available();
    if !design && !puppy_history_rows && (!puppy_tree_open || puppy_sketch || puppy_empty || !puppy_catalog) {return 1;}
    let window=load64(puppy_device);let width=puppy_objects_width();let height=gui_height(window)-76;
    if design {width=puppy_design_width();if !puppy_design_open {width=85;height=28;}}
    if height<=0 || height>16384 {return 1;}
    if !puppy_tree_texture {
        let tex=m_take(8);store64(tex,0);render_gl(puppy_device,"glGenTextures",1,tex,0,0,0,0);
        puppy_tree_texture=render_track(puppy_device,4,gui__u32(tex));if !puppy_tree_texture {return 0;}
    }
    render_gl(puppy_device,"glBindTexture",0x0de1,puppy_tree_texture,0,0,0,0);
    if puppy_tree_dirty || puppy_tree_width!=width || puppy_tree_height!=height {if design {
            if puppy_design_open {puppy_design_panel(width,height);}else {
                let rgb=m_take(width*height*4);puppy_ui_rect(rgb,width,height,0,0,width,height,42,61,64);
                puppy_ui_text(rgb,width,height,10,19,"Design  ›",width-3);puppy_ui_upload(rgb,width,height);
            }puppy_tree_width=width;puppy_tree_height=height;puppy_tree_dirty=0;
        }else if puppy_history_rows {puppy_history_panel(width,height);}else {puppy_panel_build(width,height);}}
    render_gl(puppy_device,"glViewport",0,h-(76+height)*h/gui_height(window),width*w/gui_width(window),height*h/gui_height(window),0,0);
    return render_draw(puppy_device,puppy_toolbar_vao,4,0,3);
}
fn puppy_panel_click(x,y) {
    puppy_forget_click();let w=puppy_objects_width();let h=gui_height(load64(puppy_device))-76;y=y-76;
    let previous=puppy_definition_click_time;puppy_definition_click_time=0;puppy_copy_focus=0;
    if y>=8 && y<40 {
        if x>=w-40 {puppy_query_set("");}return puppy_search_focus(1);
    }
    puppy_search_focus(0);let project=cad_project_open(puppy_history+24);
    let parts=j_parse(puppy_catalog+24);
    if puppy_definition && h>=175 && y>=h-115 {
        let copies=puppy_part_copies(parts);puppy_copy_focus=1;
        if y>=h-37 && y<h-9 {
            if x>=10 && x<94 {return puppy_copy_isolate(project,copies);}
            if cg_count(copies)>1 && x>=102 && x<134 {return puppy_copy_choose(project,copies,puppy_copy_index-1);}
            if cg_count(copies)>1 && x>=144 && x<176 {return puppy_copy_choose(project,copies,puppy_copy_index+1);}
        }
        return 1;
    }
    if puppy_pick_id && !puppy_selected_mate && h>=175 && y>=h-115 && m_eq(j_string(cad_project_item(project),"type"),"assembly") {
        if puppy_placement_open {return puppy_placement_click(project,x,y-(h-115),w);}
        if y>=h-111 && y<h-85 && x>=w-100 && x<w-10 {puppy_placement_open=1;puppy_tree_dirty=1;return 1;}
    }
    if puppy_pick_id && y>=h-37 && y<h-9 && h>=175 && m_eq(j_string(cad_project_item(project),"type"),"assembly") {
        if puppy_selected_mate {
            if x>=10 && x<110 {return puppy_delete_selection(project);}
            if x>=118 && x<w-10 && x<246 {
                let field=puppy_active_field(project);if field {return puppy_number_open(project,field);}
            }return 1;
        }
        if x>=110 && x<w-10 && x<256 {return puppy_delete_selection(project);}
    }
    if y>=h-37 && y<h-9 && x>=10 && x<104 && puppy_pick_id && puppy_body_count>1 && h>=175 {
        if puppy_isolated {return puppy_isolate(project,0);}return puppy_isolate(project,puppy_pick_id+24);
    }
    if y<48 || (y-48)/28>=puppy_panel_capacity(h) {return 1;}
    let rows=puppy_panel_rows(project,parts);
    let row=cg_at(rows,puppy_panel_offset(rows,h)+(y-48)/28);if !row {return 1;}
    let kind=load64(row);let value=load64(row+16);puppy_tree_dirty=1;
    if !kind {puppy_tree_folded=puppy_tree_folded^value;return 1;}
    if kind==3 {return puppy_toggle_branch(value);}
    if kind==5 {
        puppy_mate_selection(j_string(value,"id"));return puppy_title(project,m_cat("Selected mate   ",puppy_mate_name(value)));
    }
    if kind==4 {
        let id=j_string(value,"id");let now=gui_ticks();
        let same=puppy_definition && m_eq(puppy_definition+24,id);
        if !same {puppy_clear_pick();puppy_definition=puppy_snapshot(id);puppy_copy_index=0;}
        let copies=puppy_part_copies(parts);
        if !same && puppy_isolated {
            let i=0;while i<cg_count(copies) {
                if m_eq(j_string(cg_at(copies,i),"id"),puppy_isolated+24) {puppy_copy_index=i;}i=i+1;
            }
        }
        puppy_definition_click_time=now;
        if previous && same && now-previous<=420 && cg_count(copies)==1 {return puppy_copy_isolate(project,copies);}
        return puppy_copy_choose(project,copies,puppy_copy_index);
    }
    if kind==2 {
        puppy_clear_pick();puppy_property=load64(value+8);puppy_selected=load64(value);
        return puppy_title(project,load64(row+8));
    }
    if puppy_isolated && !m_eq(puppy_isolated+24,j_string(value,"id")) {puppy_isolate(project,0);}
    puppy_select_entry(value);return puppy_title(project,m_cat("Selected ",load64(row+8)));
}
fn puppy_objects_event(event) {
    if puppy_design_available() {
        let kind=gui_event_kind(event);let x=puppy_pointer_x;let y=puppy_pointer_y-76;
        if puppy_design_menu_event(event,x,y) {return 1;}
        let inside=x>=0 && x<puppy_design_width() && y>=0;
        if !puppy_design_open {inside=x>=0 && x<85 && y>=0 && y<28;}
        if kind==GUI_MOUSE_DOWN && !puppy_button {
            if inside {
                if gui_event_button(event)==1 {if puppy_design_open {puppy_design_click(x,y);}else {puppy_design_toggle();}}
                if gui_event_button(event)==3 && puppy_design_open {return puppy_design_context(x,y);}
                return 1;
            }puppy_design_focus=0;puppy_search_focus(0);
        }
        if inside && kind==GUI_WHEEL && puppy_design_open {puppy_design_scroll=puppy_design_scroll-gui_event_dy(event)*3;puppy_tree_dirty=1;return 1;}
        if kind==GUI_KEY_DOWN && puppy_design_focus && puppy_sheet_setting<0 && !puppy_search_active && !(gui_event_modifiers(event)&0x3c3) {
            let key=gui_event_code(event);if key==40 || (key>=79 && key<=82) {return puppy_design_navigation(key);}
        }
        return puppy_search_event(event);
    }
    if !puppy_tree_open || puppy_sketch || puppy_empty || !puppy_catalog {puppy_search_focus(0);return 0;}
    let kind=gui_event_kind(event);
    let inside=puppy_pointer_x>=0 && puppy_pointer_x<puppy_objects_width() && puppy_pointer_y>=76;
    if kind==GUI_MOUSE_DOWN {
        if !inside {puppy_copy_focus=0;puppy_definition_click_time=0;puppy_search_focus(0);return 0;}
        if gui_event_button(event)==1 {puppy_panel_click(puppy_pointer_x,puppy_pointer_y);}return 1;
    }
    if kind==GUI_WHEEL && inside && !puppy_button {
        puppy_tree_scroll=puppy_tree_scroll-gui_event_dy(event)*3;puppy_tree_dirty=1;return 1;
    }
    if kind==GUI_FOCUS && !gui_event_code(event) {puppy_search_focus(0);}
    if puppy_definition && puppy_copy_focus && !puppy_search_active && kind==GUI_KEY_DOWN && !(gui_event_modifiers(event)&0x3c3) {
        let key=gui_event_code(event);
        if key==GUI_KEY_RIGHT || key==GUI_KEY_LEFT || key==40 || key==88 {
            let project=cad_project_open(puppy_history+24);let copies=puppy_part_copies(j_parse(puppy_catalog+24));
            if key==40 || key==88 {return puppy_copy_isolate(project,copies);}
            let step=1;if key==GUI_KEY_LEFT {step=-1;}
            return puppy_copy_choose(project,copies,puppy_copy_index+step);
        }
    }
    return puppy_search_event(event);
}
fn puppy_search_event(event) {
    if !puppy_search_active {return 0;}let kind=gui_event_kind(event);
    if kind==GUI_FOCUS && !gui_event_code(event) {puppy_search_focus(0);return 0;}
    if kind==GUI_TEXT {
        let text=m_cat(puppy_query(),gui_event_text(event));if m_len(text)<=120 {puppy_query_set(text);}return 1;
    }
    if kind==GUI_IME {
        let data=gui_ime_data(gui_event_code(event));
        if data && load64(data+64) && !load64(data+80) {
            let text=m_cat(puppy_query(),load64(data+96));if m_len(text)<=120 {puppy_query_set(text);}
        }
        return 1;
    }
    if kind==GUI_KEY_DOWN {
        let key=gui_event_code(event);
        if key==60 && puppy_design_available() {return puppy_design_toggle();}
        if key==GUI_KEY_ESCAPE || key==40 {return puppy_search_focus(0);}
        if key==42 {
            let text=puppy_query();let n=m_len(text);
            if n {n=n-1;while n>0 && (load8(text+n)&192)==128 {n=n-1;}puppy_query_set(m_slice(text,n));}
        }
        if gui_event_modifiers(event)&192 {puppy_search_focus(0);return 0;}return 1;
    }
    return 0;
}
