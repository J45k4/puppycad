// Render the shared evaluator's world-space routes. Part meshes and exports
// stay separate from the wire, jacket and splice display geometry.
global puppy_harness_mesh=0;
fn puppy_path_vertex(buffer,point,color,low,high) {
    let v=m_take(16);let k=0;
    while k<3 {
        let n=load64(point+8*k);render_float(v+4*k,n,1000000000);
        if n<load64(low+8*k) {store64(low+8*k,n);}
        if n>load64(high+8*k) {store64(high+8*k,n);}k=k+1;
    }
    puppy_vertex_color(v,color);m_append(buffer,v,16);return 1;
}
fn puppy_path_lines(buffer,rows,jackets,low,high) {
    let route=pc_first(rows);while route {
        let color=0xe07a1a;if j_get(route,"cable_id") {color=0x267ab8;}
        let clearance=j_string(j_get(route,"clearance"),"status");
        if m_eq(j_string(route,"status"),"unresolved") || cg_j_count(j_get(route,"issues"))
            || m_eq(clearance,"interference") || m_eq(clearance,"unknown") {color=0xe0261f;}
        let shared=j_get(jackets,j_string(route,"jacket_id"));let start=-1;let end=-1;
        let range=pc_first(j_get(route,"jacket_range"));
        if shared && range && load64(range+24) {start=j_value(range);end=j_value(load64(range+24));}
        let row=pc_first(j_get(route,"points"));let prev=0;let i=0;
        while row {
            let point=cg_vector(row,0);
            if prev && (i<=start || i>end) {
                puppy_path_vertex(buffer,prev,color,low,high);puppy_path_vertex(buffer,point,color,low,high);
            }
            prev=point;row=load64(row+24);i=i+1;
        }
        route=load64(route+24);
    }
    return 1;
}
fn puppy_upload_harness(mesh,low,high) {
    let buffer=m_buffer();
    if !puppy_isolated {
        let jackets=j_get(mesh,"jackets");let jacket_ids=j_object();let row=pc_first(jackets);
        while row {
            if pc_first(j_get(row,"points")) {j_add(jacket_ids,j_string(row,"cable_id"),j_number(1));}
            row=load64(row+24);
        }
        puppy_path_lines(buffer,jackets,0,low,high);
        puppy_path_lines(buffer,j_get(mesh,"routes"),jacket_ids,low,high);
        row=pc_first(j_get(mesh,"splices"));while row {
            if m_eq(j_string(row,"status"),"positioned") {
                let point=cg_vector(j_get(row,"at"),0);let k=0;while k<24 {
                    let base=load64(point+k);store64(point+k,base-650000);
                    puppy_path_vertex(buffer,point,0x8c30ad,low,high);store64(point+k,base+650000);
                    puppy_path_vertex(buffer,point,0x8c30ad,low,high);store64(point+k,base);k=k+8;
                }
            }
            row=load64(row+24);
        }
    }
    let count=load64(buffer+8)/16;
    if !count && !puppy_harness_mesh {return 1;}
    if !puppy_harness_mesh {puppy_harness_mesh=puppy_colored_mesh();}
    return render_mesh_upload(puppy_harness_mesh,m_data(buffer),count,0x88e4);
}
fn puppy_draw_harness() {
    if puppy_isolated {return 1;}
    return puppy_draw_lines(puppy_harness_mesh,1);
}
fn puppy_draw_lines(mesh,mode) {
    if !mesh || !load64(mesh+32) {return 1;}
    let overlay=render_uniform(puppy_material,0,"lineOverlay");
    render_gl(puppy_device,"glUniform1i",render_uniform(puppy_material,0,"moving"),0,0,0,0,0);
    render_gl(puppy_device,"glUniform1i",overlay,mode,0,0,0,0);
    let ok=render_draw(puppy_device,load64(mesh+8),1,0,load64(mesh+32));
    render_gl(puppy_device,"glUniform1i",overlay,0,0,0,0,0);return ok;
}
fn puppy_moving_supports(rows,followers) {
    let row=pc_first(rows);while row {
        if ca_find(followers,j_string(row,"instance_id")) {return 1;}row=load64(row+24);
    }
    return 0;
}
fn puppy_harness_moves(project,followers) {
    if !puppy_harness_mesh || !load64(puppy_harness_mesh+32) || !followers {return 0;}
    let electrical=j_get(project,"electrical");if j_get(electrical,"modules") {return 1;}
    let harness=j_get(electrical,"harness");let row=pc_first(j_get(harness,"wires"));
    while row {
        let route=j_get(row,"route");
        if puppy_moving_supports(j_get(route,"anchors"),followers) || puppy_moving_supports(j_get(route,"guides"),followers) {return 1;}
        let branch=pc_first(j_get(route,"branches"));while branch {
            if puppy_moving_supports(branch,followers) {return 1;}branch=load64(branch+24);
        }
        row=load64(row+24);
    }
    row=pc_first(j_get(harness,"cables"));while row {
        let jacket=j_get(row,"jacket");
        if puppy_moving_supports(j_get(jacket,"breakouts"),followers) || puppy_moving_supports(j_get(jacket,"guides"),followers) {return 1;}
        row=load64(row+24);
    }
    row=pc_first(j_get(harness,"splices"));while row {
        if ca_find(followers,j_string(j_get(row,"placement"),"instance_id")) {return 1;}row=load64(row+24);
    }
    return 0;
}
