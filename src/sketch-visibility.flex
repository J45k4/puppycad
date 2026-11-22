// Sketch display preferences belong to the part, alongside its tree order.
fn puppy_part_tree(part) {return j_get(j_get(part,"data"),"tree");}

fn puppy_part_used_sketches(part) {
    let used=j_object();let row=pc_first(pc_nodes(part));let index=0;
    let rollback=j_get(puppy_part_tree(part),"rollbackIndex");
    while row && (!rollback || index<j_value(rollback)) {
        if pc_model_feature(row) {
            index=index+1;
            if !j_value(j_get(row,"suppressed")) {
                let id=j_string(row,"sketchId");
                if m_len(id) && !j_get(used,id) {cg_j_int(used,id,1);}
                if m_eq(j_string(row,"type"),"loft") {
                    let profile=pc_first(j_get(row,"profiles"));
                    while profile {
                        let id=j_string(profile,"sketchId");
                        if m_len(id) && !j_get(used,id) {cg_j_int(used,id,1);}
                        profile=load64(profile+24);
                    }
                }
            }
        }
        row=load64(row+24);
    }
    return used;
}

fn puppy_sketch_displayed(part,sketch,used) {
    let id=j_string(sketch,"id");
    let value=j_get(j_get(puppy_part_tree(part),"sketchVisibility"),id);
    if j_kind(value)==5 {return j_value(value);}
    return !j_get(used,id);
}

fn puppy_part_sketch_ids(part) {
    let ids=cg_array();let used=puppy_part_used_sketches(part);let sketch=pc_first(pc_nodes(part));
    while sketch {
        if m_eq(j_string(sketch,"type"),"sketch") && puppy_sketch_displayed(part,sketch,used) {
            cg_push(ids,j_string(sketch,"id"));
        }
        sketch=load64(sketch+24);
    }
    return ids;
}

fn puppy_set_sketch_visibility(project,item,sketch,visible) {
    let updated=j_parse(j_encode(project));
    let part=pc_find(j_get(updated,"items"),j_string(item,"id"));
    let id=j_string(sketch,"id");
    if !m_eq(j_string(part,"type"),"part") || !m_eq(j_string(pc_find(pc_nodes(part),id),"type"),"sketch") {return 0;}
    let tree=puppy_part_tree(part);
    if !tree {
        tree=j_object();
        j_add(j_get(part,"data"),"tree",tree);
    }
    let flags=j_get(tree,"sketchVisibility");
    if j_kind(flags)!=4 {flags=j_object();cg_j_set(tree,"sketchVisibility",flags);}
    cg_j_set(flags,id,j_new(5,visible));
    return updated;
}
