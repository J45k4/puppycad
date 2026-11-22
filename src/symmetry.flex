import "cad/project-mirror-author.flex";
import "cad/project-arc-edit.flex";

global puppy_symmetry_mode=0;
global puppy_symmetry_axis=0;
global puppy_symmetry_anchor=0;

fn puppy_symmetry_end() {
    if puppy_symmetry_axis {puppy_drop(puppy_symmetry_axis);puppy_symmetry_axis=0;}
    if puppy_symmetry_anchor {puppy_drop(puppy_symmetry_anchor);puppy_symmetry_anchor=0;}
    puppy_symmetry_mode=0;return 1;
}
fn puppy_symmetry_hint() {
    if !puppy_symmetry_axis {return "Pick the mirror line";}
    if puppy_symmetry_mode==1 {return "Pick the point, line, circle or arc to mirror";}
    if !puppy_symmetry_anchor {return "Pick the first symmetry anchor";}
    return "Pick the opposite symmetry anchor";
}
fn puppy_symmetry_key(project,key) {
    if key==GUI_KEY_ESCAPE {puppy_symmetry_end();return puppy_title(project,puppy_line_label());}
    if key==16 || key==19 {
        puppy_symmetry_end();puppy_symmetry_mode=1;if key==19 {puppy_symmetry_mode=2;}
    }
    return puppy_title(project,puppy_symmetry_hint());
}
fn puppy_segment_distance(point,a,b) {
    let dx=load64(b)-load64(a);let dy=load64(b+8)-load64(a+8);
    let px=load64(point)-load64(a);let py=load64(point+8)-load64(a+8);
    let dot=px*dx+py*dy;if dot<=0 {return pc_line_span(point,a);}
    if dot>=dx*dx+dy*dy {return pc_line_span(point,b);}
    return cg_abs(px*dy-py*dx)/pc_line_span(a,b);
}
fn puppy_mirror_hit(nodes,sk,point,only_lines) {
    let best=10000000000/puppy_draft_scale+1;let hit=0;let row=pc_first(nodes);while row {
        if m_eq(j_string(row,"sketchId"),sk) {
            let kind=pc_mirror_kind(row);let d=best;
            if kind==3 {d=puppy_segment_distance(point,pc_uv(j_get(row,"p0")),pc_uv(j_get(row,"p1")));}
            else if kind==4 && !only_lines {
                let pick=cad_sketch_circular_pick(row,point,best);if pick {d=load64(pick+8);}
            }
            else if kind && !only_lines {
                d=pc_line_span(point,pc_uv(j_get(pc_curve_geometry(row),"center")));
                if kind==1 {d=cg_abs(d-pc_circle_measure(row,"radius"));}
            }
            if d<best {best=d;hit=row;}
        }
        row=load64(row+24);
    }return hit;
}
fn puppy_symmetry_pick(project,x,y) {
    let sk=j_string(puppy_draft_sketch(project),"id");let updated=0;let label="Mirror copy added";
    let model=puppy_sketch_model(project,sk);if !model {return puppy_title(project,cg_error);}
    let point=puppy_draft_point(x,y);let nodes=pc_nodes(cad_project_part(model));
    if !puppy_symmetry_axis {
        let axis=puppy_mirror_hit(nodes,sk,point,1);
        if axis {puppy_symmetry_axis=puppy_snapshot(j_string(axis,"id"));}
        return puppy_title(project,puppy_symmetry_hint());
    }
    if puppy_symmetry_mode==1 {
        let source=puppy_mirror_hit(nodes,sk,point,0);
        if !source {return puppy_title(project,puppy_symmetry_hint());}
        updated=cad_project_mirror_entity(model,sk,j_string(source,"id"),puppy_symmetry_axis+24);
    }else {
        let anchor=puppy_distance_hit(point);if !anchor {return puppy_title(project,puppy_symmetry_hint());}
        if m_eq(j_string(anchor,"entityId"),puppy_symmetry_axis+24) {return puppy_title(project,"Choose an anchor on a different entity");}
        if !puppy_symmetry_anchor {puppy_symmetry_anchor=puppy_snapshot(j_encode(anchor));return puppy_title(project,puppy_symmetry_hint());}
        updated=cad_project_toggle_point_symmetry(model,sk,j_parse(puppy_symmetry_anchor+24),anchor,puppy_symmetry_axis+24);
        label="Point symmetry updated";
    }
    updated=puppy_line_update(project,updated);if !updated {return puppy_title(project,cg_error);}
    puppy_symmetry_end();return puppy_title(updated,label);
}
