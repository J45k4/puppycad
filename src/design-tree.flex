// The design tree reads authored records, including sketches with no evaluated body.
// Selection and expansion use document-scoped keys. They never enter model history.
global puppy_design_open=1;
global puppy_design_focus=0;
global puppy_design_selection=0;
global puppy_design_entity=0;
global puppy_design_folds=0;
global puppy_design_scroll=0;

fn puppy_design_available() {
    return !puppy_plane_chooser && !puppy_reader_report && !puppy_tree_open && !puppy_history_rows && !puppy_step_input && !puppy_mating
        && !puppy_line_report && !puppy_number_text && !puppy_loft_open && !puppy_patterns && !puppy_edge_kind && !puppy_construction_ids;
}
fn puppy_design_toggle() {
    puppy_design_focus=0;puppy_search_focus(0);puppy_query_set("");puppy_design_open=puppy_tree_open || !puppy_design_open;puppy_tree_open=0;
    puppy_tree_dirty=1;return 1;
}
fn puppy_design_width() {
    let w=gui_width(load64(puppy_device));return puppy_limit(w/3,130,254);
}
fn puppy_design_name(node,fallback) {
    let name=j_string(node,"name");if m_len(name) {return name;}
    let id=j_string(node,"id");if m_len(id) {return id;}return fallback;
}
// Each row holds the label, key, kind, document record, parent, depth,
// initial expansion, child flag, search match, current expansion and visibility.
fn puppy_design_key(kind,id) {return m_cat(kind,m_cat(m_int(m_len(id)),m_cat("/",id)));}
fn puppy_design_row(rows,parent,kind,node,label,open) {
    let id=j_string(node,"id");if !m_len(id) {id=label;}
    let key=puppy_design_key(kind,id);
    let depth=0;if parent {key=m_cat(load64(parent+8),m_cat("/",key));depth=load64(parent+40)+1;store64(parent+56,1);}
    let row=m_take(88);gui__zero(row,88);
    store64(row,label);store64(row+8,key);store64(row+16,kind);store64(row+24,node);store64(row+32,parent);
    store64(row+40,depth);store64(row+48,open);cg_push(rows,row);return row;
}
fn puppy_design_list(rows,parent,records,kind) {
    let node=pc_first(records);while node {
        puppy_design_row(rows,parent,kind,node,puppy_design_name(node,kind),0);node=load64(node+24);
    }
}
fn puppy_design_group(rows,parent,records,kind,label,open) {
    if !pc_first(records) {return 0;}
    let group=puppy_design_row(rows,parent,"group",0,label,open);puppy_design_list(rows,group,records,kind);return group;
}
fn puppy_design_node(rows,parent,node,nodes) {
    let kind=j_string(node,"type");let name=puppy_design_name(node,kind);
    let row=puppy_design_row(rows,parent,kind,node,name,0);
    if !m_eq(kind,"sketch") {return row;}
    let id=j_string(node,"id");let entity=pc_first(nodes);while entity {
        let type=j_string(entity,"type");
        if m_eq(j_string(entity,"sketchId"),id) && !pc_model_feature(entity) {
            let geom=j_string(j_get(entity,"primitive"),"type");if !m_len(geom) {geom=type;if m_len(type)>6 && m_eq(m_slice(type,6),"sketch") {geom=type+6;}}
            if m_eq(geom,"CornerRectangle") {geom="Corner rectangle";}
            if pc_capsule_entity(entity) {geom="Slot";}
            if pc_polygon_entity(entity) {geom="Polygon";}
            let name=j_string(entity,"name");if m_len(name) {geom=name;}
            puppy_design_row(rows,row,"geometry",entity,geom,0);
        }entity=load64(entity+24);
    }
    puppy_design_group(rows,row,j_get(node,"relations"),"relation","Constraints",0);
    puppy_design_group(rows,row,j_get(node,"variables"),"variable","Variables",0);
    puppy_design_group(rows,row,j_get(node,"dimensions"),"dimension","Dimensions",0);
    return row;
}
fn puppy_design_part(rows,parent,part) {
    let nodes=pc_nodes(part);let ordered=j_get(j_get(j_get(part,"data"),"tree"),"orderedNodeIds");
    let n=pc_first(nodes);let planes=0;let used=j_object();
    while n {
        if m_eq(j_string(n,"type"),"referencePlane") {
            if !planes {planes=puppy_design_row(rows,parent,"group",0,"Reference planes",0);}
            puppy_design_row(rows,planes,"plane",n,puppy_design_name(n,"Plane"),0);
        }n=load64(n+24);
    }
    let ref=pc_first(ordered);while ref {
        let id=j_value(ref);let node=pc_find(nodes,id);
        if node && !j_get(used,id) {puppy_design_node(rows,parent,node,nodes);j_add(used,id,j_number(1));}
        ref=load64(ref+24);
    }
    // Older documents may omit the UI order while still containing a valid graph.
    n=pc_first(nodes);while n {
        if (pc_model_feature(n) || m_eq(j_string(n,"type"),"sketch")) && !j_get(used,j_string(n,"id")) {puppy_design_node(rows,parent,n,nodes);}
        n=load64(n+24);
    }
}
fn puppy_design_assembly(rows,parent,data) {
    puppy_design_group(rows,parent,j_get(data,"instances"),"instance","Occurrences",1);
    puppy_design_group(rows,parent,j_get(data,"mates"),"mate","Mates",0);
    puppy_design_group(rows,parent,j_get(data,"patterns"),"pattern","Patterns",0);
    puppy_design_group(rows,parent,j_get(data,"actuators"),"actuator","Actuators",0);
    puppy_design_group(rows,parent,j_get(data,"connectors"),"connector","Connectors",0);
}
fn puppy_design_studio(rows,project) {
    let parts=puppy_design_row(rows,0,"group",0,"Parts",1);let part=pc_first(j_get(project,"parts"));
    let nodes=j_get(project,"nodes");
    while part {
        let row=puppy_design_row(rows,parts,"definition",part,puppy_design_name(part,"Part"),0);
        let feature=pc_find(nodes,j_string(part,"node"));
        if feature {puppy_design_row(rows,row,"feature",feature,puppy_design_name(feature,"Feature"),0);}
        puppy_design_group(rows,row,j_get(part,"datums"),"datum","Datums",0);
        part=load64(part+24);
    }
    puppy_design_group(rows,0,nodes,"feature","Features",0);
    let assembly=j_get(project,"assembly");
    if assembly {puppy_design_assembly(rows,puppy_design_row(rows,0,"group",0,"Assembly",0),assembly);}
    let definitions=pc_first(j_get(project,"assembly_definitions"));
    if definitions {
        let group=puppy_design_row(rows,0,"group",0,"Assembly definitions",0);
        while definitions {
            let sub=puppy_design_row(rows,group,"subassembly",definitions,puppy_design_name(definitions,"Assembly"),0);
            puppy_design_assembly(rows,sub,definitions);definitions=load64(definitions+24);
        }
    }
    puppy_design_group(rows,0,j_get(project,"parameters"),"parameter","Parameters",0);
    let sheets=j_get(project,"drawings");
    if pc_first(j_get(project,"parts")) {
        let group=puppy_design_row(rows,0,"group",0,"Drawing sheets",0);let sheet=pc_first(sheets);
        while sheet {
            let row=puppy_design_row(rows,group,"sheet",sheet,puppy_design_name(sheet,"Sheet"),0);
            puppy_design_list(rows,row,j_get(sheet,"dimensions"),"dimension");sheet=load64(sheet+24);
        }
        if cg_j_count(sheets)<4 {puppy_design_row(rows,group,"add-sheet",0,"New sheet",0);}
    }
    let electrical=j_get(project,"electrical");
    if electrical {
        let group=puppy_design_row(rows,0,"group",0,"Electrical",0);
        puppy_design_group(rows,group,j_get(electrical,"boards"),"board","Boards",0);
        let pcb=j_get(electrical,"pcb");if pcb {puppy_design_row(rows,group,"board",pcb,puppy_design_name(pcb,"PCB"),0);}
        puppy_design_group(rows,group,j_get(electrical,"components"),"component","Components",0);
        puppy_design_group(rows,group,j_get(electrical,"nets"),"net","Nets",0);
        puppy_design_group(rows,group,j_get(electrical,"sheets"),"schematic","Schematics",0);
        let harness=j_get(electrical,"harness");if harness {
            let branch=puppy_design_row(rows,group,"group",0,"Harness",0);
            puppy_design_group(rows,branch,j_get(harness,"wires"),"wire","Wires",0);
            puppy_design_group(rows,branch,j_get(harness,"cables"),"cable","Cables",0);
            puppy_design_group(rows,branch,j_get(harness,"splices"),"splice","Splices",0);
        }
    }
}
fn puppy_design_rows(project) {
    let rows=cg_array();
    if cad_project_is_studio(project) {puppy_design_studio(rows,project);}else {
        let item=pc_first(j_get(project,"items"));let active=cad_project_item(project);
        while item {
            let kind=j_string(item,"type");let row=puppy_design_row(rows,0,kind,item,puppy_design_name(item,kind),item==active);
            if m_eq(kind,"part") {puppy_design_part(rows,row,item);}
            if m_eq(kind,"assembly") {puppy_design_assembly(rows,row,j_get(item,"data"));}
            item=load64(item+24);
        }
    }
    let folds=0;if puppy_design_folds {folds=j_parse(puppy_design_folds+24);}
    let query=puppy_query();let search=m_len(query);let i=0;
    while i<cg_count(rows) {
        let r=cg_at(rows,i);let match=!search || puppy_matches(load64(r),query) || puppy_matches(load64(r+16),query) || puppy_matches(j_string(load64(r+24),"type"),query) || puppy_matches(j_string(load64(r+24),"id"),query);
        if match {let a=r;while a && !load64(a+64) {store64(a+64,1);a=load64(a+32);}}
        i=i+1;
    }
    let visible=cg_array();i=0;while i<cg_count(rows) {
        let r=cg_at(rows,i);let parent=load64(r+32);let state=j_get(folds,load64(r+8));let open=load64(r+48);if state {open=j_value(state);}
        store64(r+72,open || search);
        if load64(r+64) && (!parent || (load64(parent+72) && load64(parent+80))) {store64(r+80,1);cg_push(visible,r);}
        i=i+1;
    }
    return visible;
}
fn puppy_design_selected(rows) {
    let i=0;while puppy_design_selection && i<cg_count(rows) {
        let r=cg_at(rows,i);if m_eq(load64(r+8),puppy_design_selection+24) {return r;}i=i+1;
    }return 0;
}
fn puppy_design_action(row) {
    let kind=load64(row+16);let node=load64(row+24);
    if m_eq(kind,"geometry") {
        if pc_polygon_entity(node) {return "Set sides";}
        if pc_capsule_entity(node) {return "Set width";}
    }
    if m_eq(kind,"sketch") || m_eq(kind,"geometry") {return "Edit sketch";}
    if m_eq(kind,"part") {return "Open part";}
    if m_eq(kind,"assembly") {return "Open assembly";}
    if m_eq(kind,"sheet") {return "Open sheet";}
    if m_eq(kind,"add-sheet") {return "Create sheet";}
    if m_eq(kind,j_string(node,"type")) && pc_model_feature(node) && m_eq(j_string(puppy_design_owner(row),"type"),"part") {return "Feature history";}
    return 0;
}
fn puppy_design_owner(row) {
    while load64(row+32) {row=load64(row+32);}return load64(row+24);
}
fn puppy_design_activate(project,row) {
    let kind=load64(row+16);let node=load64(row+24);
    if m_eq(kind,"sheet") {return puppy_sheet_open(project,j_string(node,"id"));}
    if m_eq(kind,"add-sheet") {return puppy_sheet_add(project);}
    let item=puppy_design_owner(row);let items=pc_first(j_get(project,"items"));let index=0;
    while items && items!=item {items=load64(items+24);index=index+1;}
    if !items {return 1;}
    let switched=cad_project_item(project)!=item;
    if switched && !puppy_open_item(project,index) {
        return puppy_title(cad_project_open(puppy_history+24),"Cannot open this item");
    }
    if m_eq(kind,"sketch") || m_eq(kind,"geometry") {
        let geometry=0;
        if m_eq(kind,"geometry") {geometry=node;node=load64(load64(row+32)+24);}
        let result=puppy_workspace_edit_sketch(project,j_string(node,"id"));
        if puppy_profile_property(geometry) {return puppy_profile_open(project,geometry);}
        return result;
    }
    if !m_eq(kind,"part") && !m_eq(kind,"assembly") && pc_model_feature(node) {
        puppy_history_begin(project);if !puppy_history_rows {return 1;}
        let features=j_parse(puppy_history_rows+24);let n=pc_first(features);let i=0;
        while n && !m_eq(j_string(n,"id"),j_string(node,"id")) {n=load64(n+24);i=i+1;}
        if n {puppy_history_index=i;}return puppy_history_select(project,features);
    }
    if !switched && !puppy_refresh(project) {return puppy_title(project,cg_error);}
    if m_eq(kind,"part") {puppy_workspace_show_part(project);}
    return puppy_title(project,m_cat("Opened ",load64(row)));
}
fn puppy_design_choose(project,row) {
    let kind=load64(row+16);let node=load64(row+24);let id=j_string(node,"id");
    if puppy_design_entity {puppy_drop(puppy_design_entity);puppy_design_entity=0;}
    if m_eq(kind,"geometry") {puppy_design_entity=puppy_snapshot(load64(row+8));}
    if puppy_design_selection {puppy_drop(puppy_design_selection);}
    puppy_design_selection=puppy_snapshot(load64(row+8));puppy_tree_dirty=1;
    puppy_clear_pick();
    if m_eq(kind,"definition") {puppy_definition=puppy_snapshot(id);puppy_copy_index=0;}
    if m_eq(kind,"instance") && (cad_project_is_studio(project) || puppy_design_owner(row)==cad_project_item(project)) {
        let p=0;if puppy_catalog && !puppy_draft && !puppy_empty {p=pc_first(j_parse(puppy_catalog+24));}
        while p {
            if m_eq(j_string(p,"id"),id) {
                puppy_select_entry(p);p=0;
            }else {p=load64(p+24);}
        }
    }
    if puppy_driving_feature(node) && puppy_design_owner(row)==cad_project_part(project) {
        let n=pc_first(pc_nodes(cad_project_part(project)));let i=0;
        while n && n!=node {if puppy_driving_feature(n) {i=i+1;}n=load64(n+24);}
        if n {puppy_selected=i;puppy_property=0;}
    }
    if cad_project_is_studio(project) && (m_eq(kind,"parameter") || m_eq(kind,"feature") || m_eq(kind,"mate")) {
        let fields=cad_studio_properties(project);let i=0;while i<cg_count(fields) && load64(cg_at(fields,i))!=node {i=i+1;}
        if i<cg_count(fields) {puppy_selected=i;}
    }
    return puppy_title(project,m_cat("Selected ",load64(row)));
}
fn puppy_design_fold(row) {
    let folds=j_object();if puppy_design_folds {folds=j_parse(puppy_design_folds+24);puppy_drop(puppy_design_folds);}
    cg_j_int(folds,load64(row+8),!load64(row+72));puppy_design_folds=puppy_snapshot(j_encode(folds));puppy_tree_dirty=1;
}
fn puppy_design_capacity(h) {return puppy_limit((h-172)/26,0,1024);}
fn puppy_design_panel(w,h) {
    let rgb=m_take(w*h*4);puppy_ui_rect(rgb,w,h,0,0,w,h,30,43,47);
    puppy_ui_rect(rgb,w,h,w-1,0,1,h,74,96,95);
    puppy_ui_text(rgb,w,h,13,23,"Design",w-35);puppy_ui_text(rgb,w,h,w-27,23,"‹",w-6);
    puppy_ui_rect(rgb,w,h,10,35,w-20,30,19,30,33);let q=puppy_query();let hint=q;
    if !m_len(q) {hint="Search design";}puppy_ui_text(rgb,w,h,17,55,hint,w-36);
    if m_len(q) {puppy_ui_text(rgb,w,h,w-31,55,"×",w-12);}
    if puppy_search_active {puppy_ui_rect(rgb,w,h,10,63,w-20,2,143,217,184);}
    let project=cad_project_open(puppy_history+24);let rows=puppy_design_rows(project);
    let count=puppy_design_capacity(h);let max=cg_count(rows)-count;if max<0 {max=0;}
    puppy_design_scroll=puppy_limit(puppy_design_scroll,0,max);let i=0;let selected=puppy_design_selected(rows);
    while i<count && i+puppy_design_scroll<cg_count(rows) {
        let r=cg_at(rows,i+puppy_design_scroll);let x=10+load64(r+40)*13;let y=72+i*26;
        if r==selected {puppy_ui_rect(rgb,w,h,5,y,w-10,25,53,76,72);puppy_ui_rect(rgb,w,h,5,y,2,25,143,217,184);}
        if load64(r+56) {
            let k=0;while k<4 {
                if load64(r+72) {puppy_ui_rect(rgb,w,h,x+k,y+10+k,8-2*k,1,153,180,174);}
                else {puppy_ui_rect(rgb,w,h,x+2+k,y+8+k,1,8-2*k,153,180,174);}
                k=k+1;
            }
        }
        let label=load64(r);if j_value(j_get(load64(r+24),"suppressed")) {label=m_cat(label," (suppressed)");}
        puppy_ui_text(rgb,w,h,x+15,y+18,label,w-8);i=i+1;
    }
    if !cg_count(rows) {puppy_ui_text(rgb,w,h,14,93,"No matches",w-8);}
    if selected {
        let node=load64(selected+24);let detail=j_string(node,"id");if !m_len(detail) {detail=load64(selected+16);}
        puppy_ui_rect(rgb,w,h,0,h-96,w,1,72,91,92);puppy_ui_text(rgb,w,h,13,h-73,load64(selected),w-8);
        puppy_ui_text(rgb,w,h,13,h-51,detail,w-8);
        let action=puppy_design_action(selected);
        if action {puppy_ui_rect(rgb,w,h,10,h-36,w-20,27,48,68,70);puppy_ui_text(rgb,w,h,18,h-17,action,w-16);}
    }
    puppy_ui_upload(rgb,w,h);return 1;
}
fn puppy_design_click(x,y) {
    let w=puppy_design_width();let h=gui_height(load64(puppy_device))-76;
    if y<31 && x>=w-45 {return puppy_design_toggle();}
    puppy_design_focus=1;
    if y>=35 && y<65 {if x>=w-37 {puppy_query_set("");}return puppy_search_focus(1);}
    puppy_search_focus(0);
    let project=cad_project_open(puppy_history+24);let rows=puppy_design_rows(project);
    if y>=h-36 && y<h-9 {
        let r=puppy_design_selected(rows);if r && puppy_design_action(r) {return puppy_design_activate(project,r);}return 1;
    }
    if y<72 || (y-72)/26>=puppy_design_capacity(h) {return 1;}
    let r=cg_at(rows,puppy_design_scroll+(y-72)/26);if !r {return 1;}
    if load64(r+56) && (x<26+load64(r+40)*13 || m_eq(load64(r+16),"group")) {puppy_design_fold(r);return 1;}
    return puppy_design_choose(project,r);
}

fn puppy_design_navigation(key) {
    let project=cad_project_open(puppy_history+24);let rows=puppy_design_rows(project);
    let count=cg_count(rows);if !count {return 1;}
    let row=puppy_design_selected(rows);let i=0;
    while row && i<count && cg_at(rows,i)!=row {i=i+1;}
    if row {
        if key==40 && puppy_design_action(row) {return puppy_design_activate(project,row);}
        if load64(row+56) && !m_len(puppy_query()) && (key==40 || (key==79 && !load64(row+72)) || (key==80 && load64(row+72))) {
            puppy_design_fold(row);return 1;
        }
        if key==80 && load64(row+32) {while i && cg_at(rows,i)!=load64(row+32) {i=i-1;}}
        if key==79 && load64(row+56) && load64(row+72) && i+1<count {i=i+1;}
        if key==81 && i+1<count {i=i+1;}
        if key==82 && i {i=i-1;}
    }
    let capacity=puppy_design_capacity(gui_height(load64(puppy_device))-76);
    if i<puppy_design_scroll {puppy_design_scroll=i;}
    if capacity && i>=puppy_design_scroll+capacity {puppy_design_scroll=i-capacity+1;}
    return puppy_design_choose(project,cg_at(rows,i));
}
