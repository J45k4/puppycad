import "cad/project-pattern-author.flex";
import "pattern-drag.flex";

global puppy_patterns=0;
global puppy_pattern_index=0;
global puppy_pattern_slot=0;
global puppy_pattern_center=0;
global puppy_pattern_sources=0;

fn puppy_pattern_selection(list) {
    if puppy_pattern_sources {puppy_drop(puppy_pattern_sources);puppy_pattern_sources=0;}
    if list && pc_first(list) {puppy_pattern_sources=puppy_snapshot(j_encode(list));}
    return 1;
}
fn puppy_pattern_rows(project) {
    let rows=cg_array();let r=pp_first(puppy_draft_sketch(project));while r {
        if pp_kind(r) {cg_push(rows,r);}r=load64(r+24);
    }return rows;
}
fn puppy_pattern_rel(project) {
    if puppy_pattern_index<0 {return 0;}let rows=puppy_pattern_rows(project);let n=cg_count(rows);
    if !n {return 0;}puppy_pattern_index=puppy_pattern_index%n;return cg_at(rows,puppy_pattern_index);
}
fn puppy_pattern_field(project) {
    let rel=puppy_pattern_rel(project);if !rel {return 0;}
    let n=cg_j_count(j_get(rel,"instances"))+1;let i=puppy_pattern_slot;let label=0;let value=0;
    if pp_kind(rel)==1 {
        let cols=pp_columns(rel,n);let p=pc_uv(j_get(rel,"step"));let q=pp_row_step(rel);
        if i==0 {label="Columns";value=cols*1000000;}if i==1 {label="Rows";value=n/cols*1000000;}
        if i==2 {label="Step X (mm)";value=load64(p);}if i==3 {label="Step Y (mm)";value=load64(p+8);}
        if i==4 {label="Row X (mm)";value=load64(q);}if i==5 {label="Row Y (mm)";value=load64(q+8);}
    }else {
        i=i%4;let c=pc_uv(j_get(rel,"center"));if i==0 {label="Count";value=n*1000000;}
        if i==1 {label="Angle (deg)";value=pc_pose_number(j_get(rel,"angle"));}
        if i==2 {label="Center X (mm)";value=load64(c);}if i==3 {label="Center Y (mm)";value=load64(c+8);}
    }return cg_v(rel,value,label);
}
fn puppy_pattern_hint(project) {
    if puppy_pattern_center {return "Pick the rotation center or a sketch anchor";}
    if puppy_pattern_sources {return m_cat(m_int(cg_j_count(j_parse(puppy_pattern_sources+24)))," selected   Choose Grid or Circular");}
    let field=puppy_pattern_field(project);if !field {return "Pick source geometry for a pattern";}
    return m_cat(j_string(load64(field),"id"),m_cat("   ",m_cat(load64(field+16),m_cat("   ",puppy_display_number(load64(field+8))))));
}
fn puppy_pattern_begin(project) {
    puppy_patterns=1;puppy_anchor=0;puppy_pattern_index=0;puppy_pattern_slot=0;puppy_pattern_center=0;
    puppy_pattern_selection(0);puppy_bar_page=0;return puppy_title(project,puppy_pattern_hint(project));
}
fn puppy_pattern_actions(project) {
    let rows=cg_array();if !puppy_pattern_center {
        if puppy_pattern_sources {
            puppy_compact_button(rows,"Grid",10,0);puppy_compact_button(rows,"Circular",6,0);
        }else if puppy_pattern_rel(project) {
            puppy_compact_button(rows,"Set value",59,0);puppy_compact_button(rows,"Next field",43,0);puppy_compact_button(rows,"Next pattern",19,0);
            if pp_kind(puppy_pattern_rel(project))==2 {puppy_compact_button(rows,"Pick center",21,0);}
        }
        puppy_compact_button(rows,"Fit",74,0);
    }
    puppy_compact_button(rows,"Done",GUI_KEY_ESCAPE,0);return rows;
}
fn puppy_pattern_accept(project,updated) {
    if !updated {return puppy_title(project,cg_error);}
    if !m_eq(j_encode(project),j_encode(updated)) {puppy_push(updated);}
    puppy_fit_sketch(updated,1);return puppy_title(updated,puppy_pattern_hint(updated));
}
fn puppy_pattern_change(project,settings) {
    let rel=puppy_pattern_rel(project);let sk=j_string(puppy_draft_sketch(project),"id");
    return cad_project_set_pattern(project,sk,j_string(rel,"id"),settings);
}
fn puppy_pattern_apply(project,text) {
    let field=puppy_pattern_field(project);if !field {return puppy_title(project,"Select a pattern");}
    let rel=load64(field);let slot=puppy_pattern_slot;let grid=pp_kind(rel)==1;let kind=1;
    if !slot || grid && slot==1 {kind=0;}else if !grid && slot==1 {kind=2;}
    cg_begin();let q=pcv_evaluate(text,kind,pcv_bind(j_get(puppy_draft_sketch(project),"variables")),0);if !q {return puppy_title(project,cg_error);}
    let value=load64(q);if f64_lt(f64_from_i64(500),f64_magnitude(value)) {return puppy_title(project,"Pattern value exceeds range");}
    let settings=j_object();
    if !kind {
        let count=f64_to_i64_round(value);if value!=f64_from_i64(count) || count<1 || count>32 {return puppy_title(project,"Enter an integer from 1 to 32");}
        let key="count";if grid {key="columns";if slot {key="rows";}}cg_j_int(settings,key,count);
    }else {
        let number=cg_json_number(f64_to_i64_round(f64_mul(value,f64_from_i64(1000000))));
        if !grid && slot==1 {cg_j_set(settings,"angle",number);}
        else {
            let key="center";let p=0;if grid {
                key="step";if slot<4 {p=pc_uv(j_get(rel,key));}else {key="rowStep";p=pp_row_step(rel);}
            }else {p=pc_uv(j_get(rel,"center"));j_add(settings,"centerAnchor",j_new(0,0));}
            let xy=pc_json_point(p);let axis="x";if slot%2 {axis="y";}cg_j_set(xy,axis,number);j_add(settings,key,xy);
        }
    }
    let updated=puppy_pattern_change(project,settings);if !updated {return puppy_title(project,cg_error);}
    puppy_number_close();return puppy_pattern_accept(project,updated);
}
fn puppy_pattern_key(project,key) {
    if key==GUI_KEY_ESCAPE {
        if puppy_pattern_center {puppy_pattern_center=0;}else {puppy_patterns=0;puppy_pattern_selection(0);return puppy_title(project,puppy_draft_hint());}
    }else if key==74 {puppy_fit_sketch(project,0);}
    else if (key==10 || key==6) && puppy_pattern_sources {
        let settings=j_object();let kind="linearPattern";if key==6 {kind="circularPattern";}
        cg_j_text(settings,"type",kind);let sk=j_string(puppy_draft_sketch(project),"id");
        let updated=cad_project_add_pattern(project,sk,j_parse(puppy_pattern_sources+24),settings);
        if !updated {return puppy_title(project,cg_error);}puppy_pattern_index=cg_count(puppy_pattern_rows(updated))-1;
        puppy_pattern_slot=0;puppy_pattern_selection(0);return puppy_pattern_accept(project,updated);
    }else if key==19 {puppy_pattern_selection(0);puppy_pattern_index=puppy_pattern_index+1;puppy_pattern_slot=0;puppy_pattern_center=0;}
    else if !puppy_pattern_center && puppy_pattern_rel(project) {
        if key==59 {return puppy_number_open(project,puppy_pattern_field(project));}
        if key==21 && pp_kind(puppy_pattern_rel(project))==2 {puppy_pattern_center=1;}
        if key==43 {let n=6;if pp_kind(puppy_pattern_rel(project))==2 {n=4;}puppy_pattern_slot=(puppy_pattern_slot+1)%n;}
    }
    return puppy_title(project,puppy_pattern_hint(project));
}
fn puppy_pattern_candidate(best,id,key,coord,point) {
    if !coord {return 0;}let position=pc_uv(coord);
    let dx=(load64(position)-load64(point))*puppy_draft_scale/1000000;
    let dy=(load64(position+8)-load64(point+8))*puppy_draft_scale/1000000;
    if cg_abs(dx)>10000 || cg_abs(dy)>10000 {return 0;}
    let distance=dx*dx+dy*dy;if distance>=load64(best) {return 0;}
    let anchor=j_object();cg_j_text(anchor,"entityId",id);cg_j_text(anchor,"point",key);
    store64(best,distance);store64(best+8,anchor);return 1;
}
fn puppy_pattern_anchor(nodes,sk,point) {
    let row=pc_first(nodes);let best=cg_v(100000001,0,0);
    while row {
        if m_eq(j_string(row,"sketchId"),sk) {
            let type=j_string(row,"type");let id=j_string(row,"id");
            if m_eq(type,"sketchLine") {
                puppy_pattern_candidate(best,id,"p0",j_get(row,"p0"),point);
                puppy_pattern_candidate(best,id,"p1",j_get(row,"p1"),point);
            }else if m_eq(type,"sketchSpline") {
                let p=pc_first(j_get(j_get(row,"spline"),"points"));let i=0;
                while p {puppy_pattern_candidate(best,id,m_cat("point",m_int(i)),p,point);p=load64(p+24);i=i+1;}
            }else {
                let geom=pc_curve_geometry(row);let k=j_string(geom,"type");
                if pc_center_entity(row) || m_eq(k,"ellipse") {puppy_pattern_candidate(best,id,"center",j_get(geom,"center"),point);}
            }
        }row=load64(row+24);
    }return load64(best+8);
}
fn puppy_pattern_contains(rel,id) {
    let s=pc_first(j_get(rel,"sources"));while s {if m_eq(j_value(s),id) {return 1;}s=load64(s+24);}
    let g=pc_first(j_get(rel,"instances"));while g {s=pc_first(g);while s {if m_eq(j_value(s),id) {return 1;}s=load64(s+24);}g=load64(g+24);}return 0;
}
fn puppy_pattern_click(project,x,y) {
    let pt=puppy_draft_point(x,y);let nodes=pc_nodes(cad_project_part(project));let sk=j_string(puppy_draft_sketch(project),"id");
    if puppy_pattern_center {
        let a=puppy_pattern_anchor(nodes,sk,pt);let settings=j_object();j_add(settings,"center",pc_json_point(pt));
        if !a {a=j_new(0,0);}j_add(settings,"centerAnchor",a);let updated=puppy_pattern_change(project,settings);
        if !updated {return puppy_title(project,cg_error);}puppy_pattern_center=0;return puppy_pattern_accept(project,updated);
    }
    let hit=puppy_sketch_hit(nodes,sk,pt);if !hit {return puppy_title(project,puppy_pattern_hint(project));}
    let id=j_string(hit,"id");let rows=puppy_pattern_rows(project);let i=0;
    while i<cg_count(rows) {
        if puppy_pattern_contains(cg_at(rows,i),id) {puppy_pattern_index=i;puppy_pattern_slot=0;puppy_pattern_selection(0);return puppy_title(project,puppy_pattern_hint(project));}i=i+1;
    }
    let list=j_array();if puppy_pattern_sources {list=j_parse(puppy_pattern_sources+24);}
    let s=pc_first(list);while s && !m_eq(j_value(s),id) {s=load64(s+24);}
    if s {pc_line_unlink(list,s,id);}else {j_add(list,0,j_text(id));}
    puppy_pattern_index=-1;puppy_pattern_selection(list);return puppy_title(project,puppy_pattern_hint(project));
}
