// The design tree reads authored records, including sketches with no evaluated body.
// Selection and expansion use document-scoped keys. They never enter model history.
import "sketch-visibility.flex";
import "connector-editor.flex";
import "assembly-choice.flex";

global puppy_design_open=1;
global puppy_design_focus=0;
global puppy_design_selection=0;
global puppy_design_entity=0;
global puppy_design_folds=0;
global puppy_design_scroll=0;
global puppy_design_menu_count=0;
global puppy_design_menu_x=0;
global puppy_design_menu_y=0;
global puppy_design_menu_choice=0;
global puppy_design_eye_row=-1;
global PUPPY_DESIGN_MENU_WIDTH=148;
global PUPPY_DESIGN_MENU_ROW=26;

fn puppy_design_available() {
    return !puppy_plane_chooser && !puppy_support_stage && !puppy_reader_report && !puppy_tree_open && !puppy_history_rows && !puppy_step_input && !puppy_mating
        && !puppy_line_report && !puppy_number_text && !puppy_loft_open && !puppy_patterns && !puppy_edge_kind && !puppy_construction_ids;
}
fn puppy_design_toggle() {
    if puppy_assembly_choice_kind {return puppy_assembly_choice_cancel(cad_project_open(puppy_history+24));}
    puppy_design_menu_close();puppy_design_eye_clear();
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
fn puppy_design_item_name(item,kind) {
    let name=puppy_design_name(item,kind);
    if !m_eq(j_string(item,"id"),"native-part-1") || !m_eq(kind,"part") {return name;}
    let base=puppy_path;let p=puppy_path;
    while load8(p) {if load8(p)==47 {base=p+1;}p=p+1;}
    let end=p-base;
    if end>=5 && m_eq(p-5,".pcad") {end=end-5;}
    if m_eq(name,m_slice(base,end)) {return "Part 1";}
    return name;
}
// Each row holds the label, key, kind, document record, parent, depth,
// initial expansion, child flag, search match, current expansion, row visibility
// and whether the sketch is shown in Part.
fn puppy_design_key(kind,id) {return m_cat(kind,m_cat(m_int(m_len(id)),m_cat("/",id)));}
fn puppy_design_row(rows,parent,kind,node,label,open) {
    let id=j_string(node,"id");if !m_len(id) {id=label;}
    let key=puppy_design_key(kind,id);
    if m_eq(kind,"connector") {key=puppy_connector_key(j_string(node,"instanceId"),id);}
    let depth=0;if parent {key=m_cat(load64(parent+8),m_cat("/",key));depth=load64(parent+40)+1;store64(parent+56,1);}
    let row=m_take(96);gui__zero(row,96);
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
fn puppy_design_node(rows,parent,node,nodes,used) {
    let kind=j_string(node,"type");let name=puppy_design_name(node,kind);
    let row=puppy_design_row(rows,parent,kind,node,name,0);
    if !m_eq(kind,"sketch") {return row;}
    store64(row+88,puppy_sketch_displayed(load64(parent+24),node,used));
    let projected=puppy_projected_entities(node);
    let id=j_string(node,"id");let entity=pc_first(nodes);while entity {
        let type=j_string(entity,"type");
        if m_eq(j_string(entity,"sketchId"),id) && !pc_model_feature(entity) {
            let geom=j_string(j_get(entity,"primitive"),"type");if !m_len(geom) {geom=type;if m_len(type)>6 && m_eq(m_slice(type,6),"sketch") {geom=type+6;}}
            if m_eq(geom,"CornerRectangle") {geom="Corner rectangle";}
            if pc_capsule_entity(entity) {geom="Slot";}
            if pc_polygon_entity(entity) {geom="Polygon";}
            if j_get(projected,j_string(entity,"id")) {
                let kind=pc_projection_kind(entity);
                geom="Projected line";
                if kind==2 {geom="Projected circle";}else if kind==3 {geom="Projected arc";}
            }
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
    if pc_legacy_solid_part(part) {
        let data=j_get(part,"data");let step=pc_first(pc_legacy_solid_steps(data));
        while step {
            puppy_design_row(rows,parent,"legacy-step",step,pc_legacy_step_name(data,step),0);
            step=load64(step+24);
        }
        let feature=pc_first(j_get(data,"features"));
        while feature {
            if m_eq(j_string(feature,"type"),"sketch") {
                let row=puppy_design_row(rows,parent,"sketch",feature,puppy_design_name(feature,"Sketch"),0);
                puppy_design_list(rows,row,j_get(feature,"entities"),"geometry");
                puppy_design_group(rows,row,j_get(feature,"relations"),"relation","Constraints",0);
                puppy_design_group(rows,row,j_get(feature,"variables"),"variable","Variables",0);
            }
            feature=load64(feature+24);
        }
        return 1;
    }
    let nodes=pc_nodes(part);let ordered=j_get(j_get(j_get(part,"data"),"tree"),"orderedNodeIds");
    let n=pc_first(nodes);let planes=0;let used=j_object();let sketch_uses=puppy_part_used_sketches(part);
    while n {
        if m_eq(j_string(n,"type"),"referencePlane") {
            if !planes {planes=puppy_design_row(rows,parent,"group",0,"Reference planes",0);}
            puppy_design_row(rows,planes,"plane",n,puppy_design_name(n,"Plane"),0);
        }n=load64(n+24);
    }
    let ref=pc_first(ordered);while ref {
        let id=j_value(ref);let node=pc_find(nodes,id);
        if node && !j_get(used,id) {puppy_design_node(rows,parent,node,nodes,sketch_uses);j_add(used,id,j_number(1));}
        ref=load64(ref+24);
    }
    // Older documents may omit the UI order while still containing a valid graph.
    n=pc_first(nodes);while n {
        if (pc_model_feature(n) || m_eq(j_string(n,"type"),"sketch")) && !j_get(used,j_string(n,"id")) {puppy_design_node(rows,parent,n,nodes,sketch_uses);}
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
    if puppy_assembly_choice_kind {puppy_assembly_choice_rows(rows,project);
    }else if cad_project_is_studio(project) {puppy_design_studio(rows,project);}else {
        let item=pc_first(j_get(project,"items"));let active=cad_project_item(project);
        while item {
            let kind=j_string(item,"type");let row=puppy_design_row(rows,0,kind,item,puppy_design_item_name(item,kind),item==active);
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
    if m_eq(kind,"feature") || m_eq(kind,"definition") || m_eq(kind,"legacy-step") {return "Feature history";}
    if puppy_connector_row(row) {return "Edit connector";}
    if puppy_replace_row(row) {return "Replace part";}
    if m_eq(kind,j_string(node,"type")) && pc_model_feature(node) && m_eq(j_string(puppy_design_owner(row),"type"),"part") {return "Feature history";}
    return 0;
}
fn puppy_design_owner(row) {
    while load64(row+32) {row=load64(row+32);}return load64(row+24);
}
fn puppy_design_sketch_row(row) {
    return m_eq(load64(row+16),"sketch") && !pc_legacy_solid_part(puppy_design_owner(row));
}
fn puppy_design_toggle_sketch(project,row) {
    let visible=!load64(row+88);
    let updated=puppy_set_sketch_visibility(project,puppy_design_owner(row),load64(row+24),visible);
    if !updated {return 1;}
    puppy_push(updated);puppy_tree_dirty=1;puppy_part_sketch_dirty=1;
    let status="Hidden ";if visible {status="Shown ";}
    return puppy_title(updated,m_cat(status,load64(row)));
}
fn puppy_design_activate(project,row) {
    let kind=load64(row+16);let node=load64(row+24);
    if m_eq(kind,"sheet") {return puppy_sheet_open(project,j_string(node,"id"));}
    if m_eq(kind,"add-sheet") {return puppy_sheet_add(project);}
    if cad_project_is_studio(project) && (m_eq(kind,"feature") || m_eq(kind,"definition")) {
        let id="";if m_eq(kind,"feature") {id=j_string(node,"id");}
        return puppy_history_begin_at(project,id,puppy_design_history_part(project,row));
    }
    let item=puppy_design_owner(row);let index=puppy_item_index(project,item);
    if index<0 {return 1;}
    let switched=cad_project_item(project)!=item;
    if switched && !puppy_open_item(project,index) {
        return puppy_title(cad_project_open(puppy_history+24),"Cannot open this item");
    }
    if puppy_connector_row(row) {return puppy_connector_begin(project,row,0);}
    if puppy_replace_row(row) {return puppy_replace_begin(project,node);}
    if m_eq(kind,"sketch") || m_eq(kind,"geometry") {
        let geometry=0;
        if m_eq(kind,"geometry") {geometry=node;node=load64(load64(row+32)+24);}
        let result=puppy_workspace_edit_sketch(project,j_string(node,"id"));
        if puppy_profile_property(geometry) {return puppy_profile_open(project,geometry);}
        return result;
    }
    if !m_eq(kind,"part") && !m_eq(kind,"assembly") && pc_model_feature(node) {
        return puppy_history_begin_at(project,j_string(node,"id"),0);
    }
    if !switched && !puppy_refresh(project) {return puppy_title(project,cg_error);}
    if m_eq(kind,"part") {puppy_workspace_show_part(project);}
    return puppy_title(project,m_cat("Opened ",load64(row)));
}
fn puppy_design_select(key) {
    if puppy_design_entity {puppy_drop(puppy_design_entity);puppy_design_entity=0;}
    if puppy_design_selection {puppy_drop(puppy_design_selection);}
    puppy_design_selection=puppy_snapshot(key);puppy_tree_dirty=1;
}
fn puppy_design_select_item(item) {
    let kind=j_string(item,"type");let id=j_string(item,"id");
    if !m_len(id) {id=puppy_design_item_name(item,kind);}
    puppy_design_select(puppy_design_key(kind,id));
}
fn puppy_design_choose(project,row) {
    let kind=load64(row+16);let node=load64(row+24);let id=j_string(node,"id");
    puppy_design_select(load64(row+8));
    if m_eq(kind,"geometry") {puppy_design_entity=puppy_snapshot(load64(row+8));}
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
fn puppy_design_hit(rows,y,h) {
    if y<72 || (y-72)/26>=puppy_design_capacity(h) {return 0;}
    return cg_at(rows,puppy_design_scroll+(y-72)/26);
}
fn puppy_design_menu_close() {
    if puppy_design_menu_count {puppy_design_menu_count=0;puppy_tree_dirty=1;}
}
fn puppy_design_shell_part(project,row) {
    let node=load64(row+24);let kind=load64(row+16);
    if m_eq(kind,"definition") {return node;}
    if m_eq(kind,"instance") {return pc_find(j_get(project,"parts"),j_string(node,"part"));}
    return 0;
}
fn puppy_design_history_part(project,row) {
    if m_eq(load64(row+16),"feature") {row=load64(row+32);}
    if row {return puppy_design_shell_part(project,row);}
    return 0;
}
fn puppy_design_menu_entries(project,row) {
    let entries=cg_array();
    if cad_project_is_studio(project) {
        if m_eq(load64(row+16),"feature") && m_eq(j_string(load64(row+24),"type"),"shell") {
            cg_push(entries,cg_v("Edit shell",6,0));
        }else if puppy_design_shell_part(project,row) && cg_j_count(j_get(project,"nodes"))<48 {
            cg_push(entries,cg_v("Shell part",5,0));
        }
    }
    let action=puppy_design_action(row);
    if action {cg_push(entries,cg_v(action,1,0));}
    if load64(row+56) && !m_len(puppy_query()) {
        let label="Expand";if load64(row+72) {label="Collapse";}
        cg_push(entries,cg_v(label,2,0));
    }
    if puppy_design_sketch_row(row) {
        let label="Show sketch";if load64(row+88) {label="Hide sketch";}
        cg_push(entries,cg_v(label,3,0));
        if !puppy_sketch && puppy_design_owner(row)==cad_project_part(project) {
            cg_push(entries,cg_v("Sketch support",9,0));
        }
    }
    if !puppy_sketch && m_eq(load64(row+16),"extrude") && puppy_design_owner(row)==cad_project_part(project) {
        cg_push(entries,cg_v("Extrusion options",4,0));
    }
    if puppy_connector_row(row) && puppy_design_owner(row)==cad_project_item(project) {
        cg_push(entries,cg_v("Rename ID",7,0));cg_push(entries,cg_v("Attach to",8,0));
    }
    return entries;
}
fn puppy_design_context(x,y) {
    if puppy_assembly_choice_kind {return 1;}
    let h=gui_height(load64(puppy_device))-76;
    let project=cad_project_open(puppy_history+24);
    let row=puppy_design_hit(puppy_design_rows(project),y,h);
    puppy_menu_rows=0;
    if !row {return 1;}
    puppy_search_focus(0);puppy_design_focus=1;puppy_design_choose(project,row);
    let count=cg_count(puppy_design_menu_entries(project,row));
    if !count {return 1;}
    let width=puppy_design_width();
    let menu_width=puppy_limit(PUPPY_DESIGN_MENU_WIDTH,1,width-4);
    puppy_design_menu_x=puppy_limit(x,2,width-menu_width-2);
    puppy_design_menu_y=puppy_limit(y+2,2,h-count*PUPPY_DESIGN_MENU_ROW-4);
    puppy_design_menu_choice=0;puppy_design_menu_count=count;
    return 1;
}
fn puppy_design_menu_activate(choice) {
    puppy_design_menu_close();
    let project=cad_project_open(puppy_history+24);
    let row=puppy_design_selected(puppy_design_rows(project));
    if row {
        let entry=cg_at(puppy_design_menu_entries(project,row),choice);
        if entry {
            let command=load64(entry+8);
            if command==1 {puppy_design_activate(project,row);}
            if command==2 {puppy_design_fold(row);}
            if command==3 {puppy_design_toggle_sketch(project,row);}
            if command==4 {puppy_extrusion_begin(project,load64(row+24));}
            if command==5 {puppy_shell_begin(project,puppy_design_shell_part(project,row),0);}
            if command==6 {puppy_shell_begin(project,0,load64(row+24));}
            if command==7 {puppy_connector_rename_begin(project,load64(row+24));}
            if command==8 {puppy_connector_attach_begin(project,load64(row+24));}
            if command==9 {puppy_support_begin(project,load64(row+24));}
        }
    }
    return 1;
}
fn puppy_design_eye_hit(x,w) {return x>=w-38 && x<w-5;}
fn puppy_design_eye_clear() {
    if puppy_design_eye_row<0 {return 0;}
    puppy_design_eye_row=-1;puppy_tree_dirty=1;return 1;
}
fn puppy_design_eye_event(event,x,y) {
    let kind=gui_event_kind(event);
    if kind==GUI_MOUSE_MOVE {
        let row=-1;
        if puppy_design_open && !puppy_button && y>=72 && puppy_design_eye_hit(x,puppy_design_width()) {
            let height=gui_height(load64(puppy_device))-76;let index=(y-72)/26;
            if index<puppy_design_capacity(height) {row=index;}
        }
        if row==puppy_design_eye_row {return 0;}
        puppy_design_eye_row=row;puppy_tree_dirty=1;
        return !puppy_button && !puppy_anchor;
    }
    if kind==GUI_FOCUS && !gui_event_code(event) {
        let changed=puppy_design_eye_clear();return changed && !puppy_button && !puppy_search_active;
    }
    if kind==GUI_MOUSE_DOWN || kind==GUI_WHEEL || kind==GUI_RESIZE || kind==GUI_KEY_DOWN {puppy_design_eye_clear();}
    return 0;
}
fn puppy_design_menu_event(event,x,y) {
    if !puppy_design_menu_count {return puppy_design_eye_event(event,x,y);}
    let kind=gui_event_kind(event);
    if kind==GUI_MOUSE_DOWN || kind==GUI_MOUSE_MOVE {
        let width=puppy_limit(PUPPY_DESIGN_MENU_WIDTH,1,puppy_design_width()-4);
        let dy=y-puppy_design_menu_y-1;
        let inside=x>puppy_design_menu_x && x<puppy_design_menu_x+width-1 && dy>=0 && dy<puppy_design_menu_count*PUPPY_DESIGN_MENU_ROW;
        if kind==GUI_MOUSE_MOVE {
            if inside {puppy_design_menu_choice=dy/PUPPY_DESIGN_MENU_ROW;puppy_tree_dirty=1;}
            return 1;
        }
        if inside && gui_event_button(event)==1 {return puppy_design_menu_activate(dy/PUPPY_DESIGN_MENU_ROW);}
        puppy_design_menu_close();
        if gui_event_button(event)==3 && x>=0 && x<puppy_design_width() && y>=0 {return puppy_design_context(x,y);}
        if gui_event_button(event)==1 && puppy_breadcrumb_target(x,y+76) {return 0;}
        return 1;
    }
    if kind==GUI_KEY_DOWN {
        let key=gui_event_code(event);
        if key==40 || key==88 {return puppy_design_menu_activate(puppy_design_menu_choice);}
        if key==GUI_KEY_ESCAPE {puppy_design_menu_close();}
        if key==GUI_KEY_UP_ARROW || key==GUI_KEY_DOWN_ARROW {
            let step=1;if key==GUI_KEY_UP_ARROW {step=puppy_design_menu_count-1;}
            puppy_design_menu_choice=(puppy_design_menu_choice+step)%puppy_design_menu_count;puppy_tree_dirty=1;
        }
        return 1;
    }
    if kind==GUI_WHEEL || kind==GUI_RESIZE || (kind==GUI_FOCUS && !gui_event_code(event)) {
        puppy_design_menu_close();return 1;
    }
    return 0;
}
fn puppy_design_menu_draw(rgb,w,h,project,row) {
    let x=puppy_design_menu_x;let y=puppy_design_menu_y;
    let width=puppy_limit(PUPPY_DESIGN_MENU_WIDTH,1,w-4);
    let height=puppy_design_menu_count*PUPPY_DESIGN_MENU_ROW+2;
    puppy_ui_rect(rgb,w,h,x+2,y+2,width,height,16,25,27);
    puppy_ui_rect(rgb,w,h,x,y,width,height,89,111,109);
    let entries=puppy_design_menu_entries(project,row);
    let i=0;while i<cg_count(entries) {
        let top=y+1+i*PUPPY_DESIGN_MENU_ROW;let label=load64(cg_at(entries,i));
        if i==puppy_design_menu_choice {puppy_ui_rect(rgb,w,h,x+1,top,width-2,PUPPY_DESIGN_MENU_ROW,53,76,72);}
        else {puppy_ui_rect(rgb,w,h,x+1,top,width-2,PUPPY_DESIGN_MENU_ROW,30,43,47);}
        puppy_ui_text(rgb,w,h,x+10,top+18,label,x+width-6);
        i=i+1;
    }
}
fn puppy_design_eye(rgb,w,h,x,y,visible) {
    let r=115;let g=139;let b=139;
    if visible {r=153;g=218;b=191;}
    puppy_ui_rect(rgb,w,h,x+5,y,7,1,r,g,b);
    puppy_ui_rect(rgb,w,h,x+5,y+8,7,1,r,g,b);
    let i=1;while i<4 {
        let inset=4-i;let width=1+(i==1);
        puppy_ui_rect(rgb,w,h,x+inset,y+i,width,1,r,g,b);
        puppy_ui_rect(rgb,w,h,x+17-inset-width,y+i,width,1,r,g,b);
        puppy_ui_rect(rgb,w,h,x+inset,y+8-i,width,1,r,g,b);
        puppy_ui_rect(rgb,w,h,x+17-inset-width,y+8-i,width,1,r,g,b);
        i=i+1;
    }
    puppy_ui_rect(rgb,w,h,x,y+4,1,1,r,g,b);
    puppy_ui_rect(rgb,w,h,x+16,y+4,1,1,r,g,b);
    puppy_ui_rect(rgb,w,h,x+7,y+3,3,3,r,g,b);
    if !visible {
        i=0;while i<11 {
            puppy_ui_rect(rgb,w,h,x+3+i,y+9-i,2,1,178,193,193);i=i+1;
        }
    }
}
fn puppy_design_eye_tooltip(rgb,w,h,y,label) {
    let width=110;let x=w-width-8;y=puppy_limit(y,2,h-28);
    puppy_ui_rect(rgb,w,h,x+2,y+2,width,24,16,25,27);
    puppy_ui_rect(rgb,w,h,x,y,width,24,89,111,109);
    puppy_ui_rect(rgb,w,h,x+1,y+1,width-2,22,23,34,37);
    puppy_ui_text(rgb,w,h,x+8,y+17,label,w-12);
}
fn puppy_design_panel(w,h) {
    let rgb=m_take(w*h*4);puppy_ui_rect(rgb,w,h,0,0,w,h,30,43,47);
    puppy_ui_rect(rgb,w,h,w-1,0,1,h,74,96,95);
    if puppy_assembly_choice_kind {puppy_ui_text(rgb,w,h,13,23,puppy_assembly_choice_title(),w-8);}
    else {puppy_ui_text(rgb,w,h,13,23,"Design",w-35);puppy_ui_text(rgb,w,h,w-27,23,"‹",w-6);}
    puppy_ui_rect(rgb,w,h,10,35,w-20,30,19,30,33);let q=puppy_query();let hint=q;
    if !m_len(q) {hint="Search design";if puppy_assembly_choice_kind {hint=puppy_assembly_choice_search();}}puppy_ui_text(rgb,w,h,17,55,hint,w-36);
    if m_len(q) {puppy_ui_text(rgb,w,h,w-31,55,"×",w-12);}
    if puppy_search_active {puppy_ui_rect(rgb,w,h,10,63,w-20,2,143,217,184);}
    let project=cad_project_open(puppy_history+24);let rows=puppy_design_rows(project);
    let count=puppy_design_capacity(h);let max=cg_count(rows)-count;if max<0 {max=0;}
    puppy_design_scroll=puppy_limit(puppy_design_scroll,0,max);let i=0;let selected=puppy_design_selected(rows);
    if puppy_assembly_choice_kind {selected=cg_at(rows,puppy_assembly_choice_cursor);}
    let eye_tip=0;let eye_y=0;
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
        let right=w-8;
        if puppy_design_sketch_row(r) {
            let visible=load64(r+88);right=w-40;
            puppy_design_eye(rgb,w,h,w-30,y+8,visible);
            if i==puppy_design_eye_row {
                eye_tip="Show sketch";if visible {eye_tip="Hide sketch";}
                eye_y=y+26;
            }
        }
        puppy_ui_text(rgb,w,h,x+15,y+18,label,right);i=i+1;
    }
    if !cg_count(rows) {puppy_ui_text(rgb,w,h,14,93,"No matches",w-8);}
    if selected && !puppy_assembly_choice_kind {
        let node=load64(selected+24);let detail=j_string(node,"id");if !m_len(detail) {detail=load64(selected+16);}
        puppy_ui_rect(rgb,w,h,0,h-96,w,1,72,91,92);
        if puppy_connector_row(selected) && puppy_design_owner(selected)==cad_project_item(project) {
            let label=load64(selected);
            if !m_eq(label,detail) {label=m_cat(label,m_cat(" · ",detail));}
            puppy_ui_text(rgb,w,h,13,h-80,label,w-8);
            puppy_connector_grid(rgb,w,h,project,selected);
        }else {
            puppy_ui_text(rgb,w,h,13,h-73,load64(selected),w-8);
            puppy_ui_text(rgb,w,h,13,h-51,detail,w-8);
            let action=puppy_design_action(selected);
            if action {puppy_ui_rect(rgb,w,h,10,h-36,w-20,27,48,68,70);puppy_ui_text(rgb,w,h,18,h-17,action,w-16);}
        }
        if puppy_design_menu_count {puppy_design_menu_draw(rgb,w,h,project,selected);}
    }
    if eye_tip && !puppy_design_menu_count {puppy_design_eye_tooltip(rgb,w,h,eye_y,eye_tip);}
    puppy_ui_upload(rgb,w,h);return 1;
}
fn puppy_design_click(x,y) {
    let w=puppy_design_width();let h=gui_height(load64(puppy_device))-76;
    if !puppy_assembly_choice_kind && y<31 && x>=w-45 {return puppy_design_toggle();}
    puppy_design_focus=1;
    if y>=35 && y<65 {if x>=w-37 {puppy_query_set("");}return puppy_search_focus(1);}
    puppy_search_focus(0);
    let project=cad_project_open(puppy_history+24);let rows=puppy_design_rows(project);
    if puppy_assembly_choice_kind {
        let row=puppy_design_hit(rows,y,h);if row {return puppy_assembly_choice_apply(project,load64(row+24));}return 1;
    }
    let selected=puppy_design_selected(rows);
    if y>=h-96 && puppy_connector_row(selected) && puppy_design_owner(selected)==cad_project_item(project) {
        return puppy_connector_click(project,selected,x,y,w,h);
    }
    if y>=h-36 && y<h-9 {
        if selected && puppy_design_action(selected) {return puppy_design_activate(project,selected);}return 1;
    }
    let r=puppy_design_hit(rows,y,h);if !r {return 1;}
    if puppy_design_sketch_row(r) && puppy_design_eye_hit(x,w) {return puppy_design_toggle_sketch(project,r);}
    if load64(r+56) && (x<26+load64(r+40)*13 || m_eq(load64(r+16),"group")) {puppy_design_fold(r);return 1;}
    puppy_design_choose(project,r);
    let kind=load64(r+16);
    if !cad_project_is_studio(project) && (m_eq(kind,"part") || m_eq(kind,"assembly") || m_eq(kind,"sketch")) {
        return puppy_design_activate(project,r);
    }
    return 1;
}

fn puppy_design_navigation(key) {
    let project=cad_project_open(puppy_history+24);let rows=puppy_design_rows(project);
    let count=cg_count(rows);if !count {return 1;}
    if puppy_assembly_choice_kind {
        let cursor=puppy_limit(puppy_assembly_choice_cursor,0,count-1);
        if key==40 {return puppy_assembly_choice_apply(project,load64(cg_at(rows,cursor)+24));}
        if key==81 {cursor=puppy_limit(puppy_assembly_choice_cursor+1,0,count-1);}
        if key==82 {cursor=puppy_limit(puppy_assembly_choice_cursor-1,0,count-1);}
        puppy_assembly_choice_cursor=cursor;
        let capacity=puppy_design_capacity(gui_height(load64(puppy_device))-76);
        if cursor<puppy_design_scroll {puppy_design_scroll=cursor;}
        if capacity && cursor>=puppy_design_scroll+capacity {puppy_design_scroll=cursor-capacity+1;}
        puppy_tree_dirty=1;return 1;
    }
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
