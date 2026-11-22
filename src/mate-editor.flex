// Only snapshots of the two local connector frames survive between gestures.
import "cad/mate-author.flex";
global puppy_mating=0;
global puppy_mate_first=0;
global puppy_mate_second=0;
global puppy_mate_selection_frame=0;
global puppy_selected_mate=0;
global puppy_first_marker=0;
global puppy_second_marker=0;

fn puppy_mate_clear() {
    if puppy_mate_first {puppy_drop(puppy_mate_first);puppy_mate_first=0;}
    if puppy_mate_second {puppy_drop(puppy_mate_second);puppy_mate_second=0;}
    puppy_mating=0;return 1;
}
fn puppy_mate_actions() {
    let rows=cg_array();
    if puppy_mate_second {
        puppy_compact_button(rows,"Fasten",30,0);puppy_compact_button(rows,"Revolute",31,0);
        puppy_compact_button(rows,"Slider",32,0);puppy_compact_button(rows,"Planar",33,0);puppy_compact_button(rows,"Ball",34,0);
    }else if puppy_pick_id {puppy_compact_button(rows,"Fasten to world",10,0);}
    puppy_compact_button(rows,"Cancel",GUI_KEY_ESCAPE,0);return rows;
}
fn puppy_mate_accept(project,updated,message) {
    if !updated {return puppy_title(project,cg_error);}
    if !puppy_refresh(updated) {return puppy_title(project,cg_error);}
    puppy_mate_clear();puppy_push(updated);return puppy_title(updated,message);
}
fn puppy_mate_ground(project) {
    if !puppy_pick_id {return puppy_title(project,"Select an instance to fasten");}
    let updated=cad_project_fasten_to_world(project,puppy_pick_id+24);
    return puppy_mate_accept(project,updated,"Fastened to world");
}
fn puppy_mate_hit(project,ray) {
    let distance=puppy_radius*puppy_zoom/100+2;
    let tolerance=puppy_limit(distance*8000000/(19*gui_height(load64(puppy_device))/10),1000,500000000);
    return pm_element_hit(project,load64(ray),load64(ray+8),puppy_visible_id(),tolerance);
}
fn puppy_mate_mark(hit,second) {
    let mesh=puppy_first_marker;if second {mesh=puppy_second_marker;}
    if !mesh {mesh=puppy_colored_mesh();if second {puppy_second_marker=mesh;}else {puppy_first_marker=mesh;}}
    if !mesh {return 0;}
    let color=0x70edcb;if second {color=0xffce67;}
    let lines=m_buffer();let low=cg_v(0,0,0);let high=cg_v(0,0,0);
    let outline=load64(hit+40);let count=0;let curve=m_eq(load64(hit+24),"Curved edge");
    if outline {count=1;if curve {count=cg_count(outline);}}
    let i=0;while i<count {
        let edge=outline;if curve {edge=cg_at(outline,i);}
        puppy_path_vertex(lines,load64(edge),color,low,high);puppy_path_vertex(lines,load64(edge+8),color,low,high);i=i+1;
    }
    let point=load64(hit);let p=cg_v(load64(point),load64(point+8),load64(point+16));
    let tick=(puppy_radius*puppy_zoom/100+2)*6500000/gui_height(load64(puppy_device));let k=0;
    while k<24 {
        store64(p+k,load64(point+k)-tick);puppy_path_vertex(lines,p,color,low,high);
        store64(p+k,load64(point+k)+tick);puppy_path_vertex(lines,p,color,low,high);store64(p+k,load64(point+k));k=k+8;
    }
    return render_mesh_upload(mesh,m_data(lines),load64(lines+8)/16,0x88e8);
}
fn puppy_mate_element(project,hit) {
    let face=pm_element_frame(project,hit,puppy_mate_first!=0);if !face {return puppy_title(project,cg_error);}
    let id=j_string(face,"instanceId");
    if puppy_mate_first && m_eq(id,j_string(j_parse(puppy_mate_first+24),"instanceId")) {
        return puppy_title(project,"Pick a vertex, edge or face on a different instance");
    }
    if !puppy_mate_mark(hit,puppy_mate_first!=0) {return puppy_title(project,"Cannot draw mate selection");}
    let entry=pc_find(j_parse(puppy_catalog+24),id);if entry {puppy_select_entry(entry);}
    if !puppy_mate_first {puppy_mate_first=puppy_snapshot(j_encode(face));return puppy_title(project,m_cat(j_string(face,"pickKind")," selected   |   Pick on the second instance"));}
    puppy_mate_second=puppy_snapshot(j_encode(face));return puppy_title(project,m_cat(j_string(face,"pickKind")," selected   |   Choose a mate type"));
}
fn puppy_mate_begin(project,hover) {
    if puppy_empty {return puppy_title(project,"Insert an instance first");}
    let in_tree=puppy_tree_open && puppy_pointer_x<puppy_objects_width();
    puppy_mating=1;puppy_menu_rows=0;puppy_search_focus(0);puppy_tree_open=0;
    if puppy_mate_selection_frame {puppy_mate_first=puppy_snapshot(puppy_mate_selection_frame+24);}
    if hover && puppy_pick_id && puppy_pointer_y>=76 && !in_tree {
        let ray=puppy_ray(puppy_pointer_x,puppy_pointer_y);let hit=0;
        if ray {hit=puppy_mate_hit(project,ray);}
        if cg_error {return puppy_title(project,cg_error);}
        if !hit {return puppy_mate_ground(project);}
        if puppy_mate_first && !m_eq(puppy_pick_id+24,load64(load64(hit+16))) {
            return puppy_mate_element(project,hit);
        }
    }
    if puppy_mate_first {return puppy_title(project,"Pick on the second instance");}
    return puppy_title(project,"Pick a vertex, edge or face on the first instance");
}
fn puppy_mate_key(project,key) {
    if key==GUI_KEY_ESCAPE {puppy_mate_clear();return puppy_title(project,"Mate cancelled");}
    if key==10 && !puppy_mate_second {return puppy_mate_ground(project);}
    if !puppy_mate_second && key>=30 && key<=35 {return puppy_standard_view(key-30);}
    if puppy_mate_second && key>=30 && key<=34 {
        let kinds=j_parse("[\"fasten\",\"revolute\",\"prismatic\",\"planar\",\"ball\"]");let kind=j_value(sr_arg(kinds,key-30));
        let updated=cad_project_add_mate(project,kind,j_parse(puppy_mate_first+24),j_parse(puppy_mate_second+24));
        return puppy_mate_accept(project,updated,m_cat("Created mate   ",kind));
    }return 1;
}
fn puppy_mate_event(event) {
    if !puppy_mating {return 0;}let kind=gui_event_kind(event);let project=cad_project_open(puppy_history+24);
    if kind==GUI_CLOSE || kind==GUI_QUIT {puppy_mate_clear();return 0;}
    if kind==GUI_KEY_DOWN {return puppy_mate_key(project,gui_event_code(event));}
    let x=gui_event_x(event);let y=gui_event_y(event);
    if kind==GUI_MOUSE_DOWN {
        let button=gui_event_button(event);
        if button==1 && y>=42 && y<68 {return puppy_action_bar(puppy_mate_actions(),0,gui_width(load64(puppy_device)),x);}
        if y>=76 {
            if button==2 || button==3 {return puppy_view_down(button,x,y);}
            if button==1 && !puppy_mate_second && !puppy_cube_click(x,y) {
                let ray=puppy_ray(x,y);if !ray {return 1;}
                let hit=puppy_mate_hit(project,ray);if !hit {
                    let message=cg_error;if !message {message="Click a visible vertex, edge or face";}return puppy_title(project,message);
                }
                return puppy_mate_element(project,hit);
            }
        }
    }
    if kind==GUI_MOUSE_MOVE && puppy_button {return puppy_view_move(x,y);}
    if puppy_button && (kind==GUI_MOUSE_UP || kind==GUI_FOCUS && !gui_event_code(event)) {return puppy_view_end(0);}
    if kind==GUI_WHEEL {puppy_zoom=puppy_limit(puppy_zoom-gui_event_dy(event)*15,120,800);}
    return kind!=GUI_RESIZE && kind!=GUI_EXPOSE;
}


fn puppy_mate_selection(id) {
    puppy_placement_open=0;puppy_placement_slot=-1;
    if puppy_selected_mate {puppy_drop(puppy_selected_mate);puppy_selected_mate=0;}
    if id {puppy_selected_mate=puppy_snapshot(id);}return 1;
}
fn puppy_mate_name(mate) {
    let label=j_string(mate,"name");if !m_len(label) {label=j_string(mate,"id");}return label;
}
fn puppy_other_instance(data,mate,id) {
    let other=j_string(j_get(mate,"a"),"instanceId");if m_eq(other,id) {other=j_string(j_get(mate,"b"),"instanceId");}
    if !m_len(other) {return "World";}let name=j_string(pc_find(j_get(data,"instances"),other),"name");
    if m_len(name) {return name;}return other;
}
fn puppy_restore_mate_pick(owner,id) {
    let part=pc_find(j_parse(puppy_catalog+24),owner);if part {puppy_select_entry(part);puppy_mate_selection(id);}return 1;
}
fn puppy_delete_selection(project) {
    if !puppy_pick_id {return puppy_title(project,"Select an instance or mate to remove");}
    let owner=m_slice(puppy_pick_id+24,m_len(puppy_pick_id+24));let result=0;let label=0;
    if puppy_selected_mate {
        let id=puppy_selected_mate+24;label=m_cat("Removed mate   ",id);result=cad_project_remove_mate(project,id);
    }else {label=m_cat("Removed instance   ",owner);result=cad_project_remove_instance(project,owner);owner=0;}
    if !result || !puppy_refresh(result) {return puppy_title(project,cg_error);}
    if owner {puppy_restore_mate_pick(owner,0);}puppy_push(result);return puppy_title(result,label);
}
