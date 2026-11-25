// Highlight the clicked connected planar face region or sharp crease edge.
global puppy_face_fill_mesh=0;
global puppy_face_edge_mesh=0;
global puppy_face_tube_mesh=0;
global puppy_face_fill_count=0;
global puppy_face_edge_count=0;
global puppy_face_tube_count=0;
global puppy_selected_face=0;
global puppy_selected_edge=0;
global puppy_selected_vertex=0;
global puppy_sel_vx=0;
global puppy_sel_vy=0;
global puppy_sel_vz=0;
global puppy_sel_vert_mesh=0;
global puppy_sel_vert_count=0;
global puppy_sel_face_normal=0;
global puppy_sel_face_normal_valid=0;
global puppy_sel_face_key=0;
global puppy_sel_edge_key=0;
global puppy_hover_verts=0;
global puppy_hover_vert_count=0;
global puppy_hover_vert_bytes=0;
global puppy_hover_vert_corner=0;
global puppy_hover_vert_corner_bytes=0;
global puppy_hover_tris=0;
global puppy_hover_tri_count=0;
global puppy_hover_tri_bytes=0;
global puppy_hover_tri_norms=0;
global puppy_hover_tri_norms_bytes=0;
global puppy_hover_raw_tri_count=0;
global puppy_hover_vis_first=0;
global puppy_hover_vis_end=0;
global puppy_hover_skverts=0;
global puppy_hover_skvert_count=0;
global puppy_hover_skvert_bytes=0;
global puppy_hover_kind=0;
global puppy_hover_index=-1;
global puppy_hover_tri_seed=-1;
global puppy_hover_mesh=0;
global puppy_hover_count=0;
global puppy_hover_face_fill_mesh=0;
global puppy_hover_face_fill_count=0;
global puppy_hover_face_edge_mesh=0;
global puppy_hover_face_edge_count=0;

fn puppy_hover_clear() {
    let changed=puppy_hover_kind!=0 || puppy_hover_index>=0 || puppy_hover_count!=0
        || puppy_hover_face_fill_count!=0 || puppy_hover_face_edge_count!=0;
    puppy_hover_kind=0;puppy_hover_index=-1;puppy_hover_tri_seed=-1;
    puppy_hover_count=0;puppy_hover_face_fill_count=0;puppy_hover_face_edge_count=0;
    return changed;
}
fn puppy_hover_free_skverts() {
    if puppy_hover_skverts {syscall(11,puppy_hover_skverts,puppy_hover_skvert_bytes,0,0,0,0);puppy_hover_skverts=0;}
    puppy_hover_skvert_count=0;puppy_hover_skvert_bytes=0;
    return 1;
}
fn puppy_hover_free_cache() {
    if puppy_hover_verts {syscall(11,puppy_hover_verts,puppy_hover_vert_bytes,0,0,0,0);puppy_hover_verts=0;}
    puppy_hover_vert_count=0;puppy_hover_vert_bytes=0;
    if puppy_hover_vert_corner {syscall(11,puppy_hover_vert_corner,puppy_hover_vert_corner_bytes,0,0,0,0);puppy_hover_vert_corner=0;}
    puppy_hover_vert_corner_bytes=0;
    if puppy_hover_tris {syscall(11,puppy_hover_tris,puppy_hover_tri_bytes,0,0,0,0);puppy_hover_tris=0;}
    puppy_hover_tri_count=0;puppy_hover_tri_bytes=0;
    if puppy_hover_tri_norms {syscall(11,puppy_hover_tri_norms,puppy_hover_tri_norms_bytes,0,0,0,0);puppy_hover_tri_norms=0;}
    puppy_hover_tri_norms_bytes=0;puppy_hover_raw_tri_count=0;
    puppy_hover_vis_first=0;puppy_hover_vis_end=0;
    return puppy_hover_clear();
}
fn puppy_face_clear() {
    puppy_face_fill_count=0;puppy_face_edge_count=0;puppy_face_tube_count=0;
    puppy_selected_vertex=0;puppy_sel_vert_count=0;puppy_sel_face_normal_valid=0;
    puppy_sel_face_key=0;puppy_sel_edge_key=0;
    if puppy_selected_face {puppy_drop(puppy_selected_face);puppy_selected_face=0;}
    if puppy_selected_edge {puppy_drop(puppy_selected_edge);puppy_selected_edge=0;}
    return 1;
}
fn puppy_face_close() {
    puppy_face_clear();puppy_hover_free_cache();puppy_hover_free_skverts();
    render_mesh_close(puppy_face_fill_mesh);render_mesh_close(puppy_face_edge_mesh);
    render_mesh_close(puppy_face_tube_mesh);render_mesh_close(puppy_hover_mesh);
    render_mesh_close(puppy_sel_vert_mesh);
    render_mesh_close(puppy_hover_face_fill_mesh);render_mesh_close(puppy_hover_face_edge_mesh);
    puppy_face_fill_mesh=0;puppy_face_edge_mesh=0;puppy_face_tube_mesh=0;puppy_hover_mesh=0;puppy_sel_vert_mesh=0;
    puppy_hover_face_fill_mesh=0;puppy_hover_face_edge_mesh=0;
    return 1;
}
fn puppy_face_point_near(a,b) {
    return cg_abs(load64(a)-load64(b))<=5000 && cg_abs(load64(a+8)-load64(b+8))<=5000
        && cg_abs(load64(a+16)-load64(b+16))<=5000;
}
fn puppy_face_point_on_seg(p,a,b) {
    if puppy_face_point_near(p,a) || puppy_face_point_near(p,b) {return -1;}
    let dx=(load64(b)-load64(a))/1000;let dy=(load64(b+8)-load64(a+8))/1000;let dz=(load64(b+16)-load64(a+16))/1000;
    let len2=dx*dx+dy*dy+dz*dz;if len2<=25 {return -1;}
    let px=(load64(p)-load64(a))/1000;let py=(load64(p+8)-load64(a+8))/1000;let pz=(load64(p+16)-load64(a+16))/1000;
    let dot=px*dx+py*dy+pz*dz;if dot<=0 || dot>=len2 {return -1;}
    let len=cg_sqrt(len2);if !len {return -1;}
    let cx=py*dz-pz*dy;let cy=pz*dx-px*dz;let cz=px*dy-py*dx;
    if cg_abs(cx)>5*len || cg_abs(cy)>5*len || cg_abs(cz)>5*len {return -1;}
    return dot;
}
fn puppy_face_vertex_on_poly(verts,p) {
    let n=cg_count(verts);let i=0;
    while i<n {
        if puppy_face_point_on_seg(p,cg_at(verts,i),cg_at(verts,(i+1)%n))>0 {return 1;}
        i=i+1;
    }
    return 0;
}
fn puppy_face_polys_touch(pa,pb) {
    let va=load64(pa);let vb=load64(pb);let na=cg_count(va);let nb=cg_count(vb);
    let i=0;while i<na {
        let a=cg_at(va,i);let j=0;
        while j<nb {
            if puppy_face_point_near(a,cg_at(vb,j)) {return 1;}
            j=j+1;
        }
        if puppy_face_vertex_on_poly(vb,a) {return 1;}
        i=i+1;
    }
    let j=0;while j<nb {
        if puppy_face_vertex_on_poly(va,cg_at(vb,j)) {return 1;}
        j=j+1;
    }
    return 0;
}
fn puppy_face_vertex(buf,p,rgb) {
    let v=m_take(16);let k=0;
    while k<3 {render_float(v+k*4,load64(p+k*8),1000000000);k=k+1;}
    puppy_vertex_color(v,rgb);return m_append(buf,v,16);
}
fn puppy_shaded_vertex(buf,p,rgb,norm_ptr) {
    let v=m_take(28);let k=0;
    while k<3 {render_float(v+k*4,load64(p+k*8),1000000000);k=k+1;}
    puppy_vertex_color(v,rgb);
    if norm_ptr {m_copy(v+16,norm_ptr,12);}else {gui__zero(v+16,12);}
    return m_append(buf,v,28);
}
fn puppy_face_seed(plane,point) {
    let count=cg_count(plane);let best=0;let best_dist=0x7fffffffffffffff;let i=0;
    while i<count {
        let verts=load64(cg_at(plane,i));let n=cg_count(verts);
        let sx=0;let sy=0;let sz=0;let j=0;
        while j<n {
            let p=cg_at(verts,j);sx=sx+load64(p)/1000;sy=sy+load64(p+8)/1000;sz=sz+load64(p+16)/1000;j=j+1;
        }
        let dx=sx/n-load64(point)/1000;let dy=sy/n-load64(point+8)/1000;let dz=sz/n-load64(point+16)/1000;
        let d=dx*dx+dy*dy+dz*dz;
        if d<best_dist {best_dist=d;best=i;}
        i=i+1;
    }
    return best;
}
fn puppy_face_add_unique(points,p) {
    let n=cg_count(points);if n>=512 {return 0;}
    let i=0;while i<n {
        if puppy_face_point_near(cg_at(points,i),p) {return 0;}
        i=i+1;
    }
    cg_push(points,p);return 1;
}
fn puppy_face_emit_split_edge(segs,all_pts,a,b) {
    if puppy_face_point_near(a,b) {return 0;}
    let hits=cg_array();let i=0;let total=cg_count(all_pts);
    while i<total {
        let p=cg_at(all_pts,i);let t=puppy_face_point_on_seg(p,a,b);
        if t>0 {cg_push(hits,cg_v(p,t,0));}
        i=i+1;
    }
    let m=cg_count(hits);let hdata=load64(hits+16);let j=1;
    while j<m {
        let key=load64(hdata+j*8);let kt=load64(key+8);let k=j-1;
        while k>=0 && load64(load64(hdata+k*8)+8)>kt {
            store64(hdata+(k+1)*8,load64(hdata+k*8));k=k-1;
        }
        store64(hdata+(k+1)*8,key);j=j+1;
    }
    let prev=a;i=0;
    while i<m {
        let next=load64(cg_at(hits,i));
        if !puppy_face_point_near(prev,next) {cg_push(segs,cg_v(prev,next,0));prev=next;}
        i=i+1;
    }
    if !puppy_face_point_near(prev,b) {cg_push(segs,cg_v(prev,b,0));}
    return 1;
}
fn puppy_sel_mesh_key(data,count,stride) {
    let sum=count*1000003+1;let i=0;
    while i<count {
        let p=data+i*stride;
        sum=sum+bytes_load32(p)+bytes_load32(p+4)*3+bytes_load32(p+8)*7;
        i=i+1;
    }
    if !sum {sum=1;}
    return sum;
}
fn puppy_face_select(project,ray,hit) {
    puppy_face_clear();
    if !hit {return 0;}
    if puppy_hover_tri_count && ray {
        let origin=load64(ray);let dir=cg_normal(load64(ray+8));
        if dir {
            let thit=puppy_hover_tri_hit(origin,dir);
            if thit {
                let tri=load64(thit);
                let planar=puppy_surface_upload_from_tri(tri,1);
                if planar>0 {
                    if planar==2 {
                        let sn=puppy_hover_tris+tri*72+24;
                        if puppy_sel_face_normal {m_copy(puppy_sel_face_normal,sn,24);puppy_sel_face_normal_valid=1;}
                        if cad_project_part(project) {
                            let face=cad_project_pick_face_in(project,origin,load64(ray+8),puppy_visible_id());
                            if face {puppy_selected_face=puppy_snapshot(j_encode(face));}
                            cg_error=0;
                        }
                    }
                    return 1;
                }
            }
        }
        cg_error=0;
    }
    let point=load64(hit);let normal=load64(hit+8);let body=load64(hit+16);
    if !body {return 0;}
    let faces=load64(body+24);if !faces {return 0;}
    let plane=cg_array();let i=0;let nfaces=cg_count(faces);
    while i<nfaces {
        let poly=cg_at(faces,i);let verts=load64(poly);
        if cg_count(verts)>=3 && cg_dot(load64(poly+8),normal)>999000000 {
            let dist=cg_dot(normal,cg_sub(cg_at(verts,0),point));
            if cg_abs(dist)<10000 {cg_push(plane,poly);}
        }
        i=i+1;
    }
    let count=cg_count(plane);if !count {return 0;}
    let marks=m_take(count*8);gui__zero(marks,count*8);
    let queue=m_take(count*8);let tail=0;i=0;
    while i<count {
        if pc_hit_polygon(cg_at(plane,i),point) {store64(marks+i*8,1);store64(queue+tail*8,i);tail=tail+1;}
        i=i+1;
    }
    if !tail {
        let seed=puppy_face_seed(plane,point);store64(marks+seed*8,1);store64(queue,seed);tail=1;
    }
    let head=0;
    while head<tail {
        let current=cg_at(plane,load64(queue+head*8));let v=0;
        while v<count {
            if !load64(marks+v*8) && puppy_face_polys_touch(current,cg_at(plane,v)) {
                store64(marks+v*8,1);store64(queue+tail*8,v);tail=tail+1;
            }
            v=v+1;
        }
        head=head+1;
    }
    let fill_buf=m_buffer();let all_pts=cg_array();
    let sel_count=tail;i=0;
    while i<sel_count {
        let verts=load64(cg_at(plane,load64(queue+i*8)));let nv=cg_count(verts);
        let j=0;while j<nv {puppy_face_add_unique(all_pts,cg_at(verts,j));j=j+1;}
        let v0=cg_at(verts,0);j=1;
        while j+1<nv {
            puppy_shaded_vertex(fill_buf,v0,0xd48e18,0);
            puppy_shaded_vertex(fill_buf,cg_at(verts,j),0xd48e18,0);
            puppy_shaded_vertex(fill_buf,cg_at(verts,j+1),0xd48e18,0);
            j=j+1;
        }
        i=i+1;
    }
    let fill_verts=load64(fill_buf+8)/28;if !fill_verts {return 0;}
    let segs=cg_array();i=0;
    while i<sel_count {
        let verts=load64(cg_at(plane,load64(queue+i*8)));let nv=cg_count(verts);let j=0;
        while j<nv {
            puppy_face_emit_split_edge(segs,all_pts,cg_at(verts,j),cg_at(verts,(j+1)%nv));
            j=j+1;
        }
        i=i+1;
    }
    let nsegs=cg_count(segs);let cancelled=m_take((nsegs+1)*8);gui__zero(cancelled,(nsegs+1)*8);
    i=0;while i<nsegs {
        if !load64(cancelled+i*8) {
            let si=cg_at(segs,i);let ui=load64(si);let vi=load64(si+8);let j=i+1;
            while j<nsegs {
                if !load64(cancelled+j*8) {
                    let sj=cg_at(segs,j);
                    if puppy_face_point_near(ui,load64(sj+8)) && puppy_face_point_near(vi,load64(sj)) {
                        store64(cancelled+i*8,1);store64(cancelled+j*8,1);j=nsegs;
                    }
                }
                j=j+1;
            }
        }
        i=i+1;
    }
    let edge_buf=m_buffer();i=0;
    while i<nsegs {
        if !load64(cancelled+i*8) {
            let s=cg_at(segs,i);
            puppy_face_vertex(edge_buf,load64(s),0xffa600);
            puppy_face_vertex(edge_buf,load64(s+8),0xffa600);
        }
        i=i+1;
    }
    let edge_verts=load64(edge_buf+8)/16;
    if !puppy_face_fill_mesh {puppy_face_fill_mesh=puppy_shaded_mesh();}
    if !render_mesh_upload(puppy_face_fill_mesh,m_data(fill_buf),fill_verts,0x88e4) {return 0;}
    puppy_face_fill_count=fill_verts;
    puppy_sel_face_key=puppy_sel_mesh_key(m_data(fill_buf),fill_verts,28);
    if puppy_sel_face_normal {m_copy(puppy_sel_face_normal,normal,24);puppy_sel_face_normal_valid=1;}
    if edge_verts {
        if !puppy_face_edge_mesh {puppy_face_edge_mesh=puppy_colored_mesh();}
        if render_mesh_upload(puppy_face_edge_mesh,m_data(edge_buf),edge_verts,0x88e4) {
            puppy_face_edge_count=edge_verts;
        }
    }
    if cad_project_part(project) && ray {
        let face=cad_project_pick_face_in(project,load64(ray),load64(ray+8),puppy_visible_id());
        if face {puppy_selected_face=puppy_snapshot(j_encode(face));}
        cg_error=0;
    }
    return 1;
}
fn puppy_edge_offset(p,axis,rad) {
    return cg_v(load64(p)+load64(axis)*rad/1000000000,
                load64(p+8)+load64(axis+8)*rad/1000000000,
                load64(p+16)+load64(axis+16)*rad/1000000000);
}
fn puppy_edge_quad(buf,a0,a1,b1,b0,rgb) {
    puppy_face_vertex(buf,a0,rgb);puppy_face_vertex(buf,a1,rgb);puppy_face_vertex(buf,b1,rgb);
    puppy_face_vertex(buf,a0,rgb);puppy_face_vertex(buf,b1,rgb);return puppy_face_vertex(buf,b0,rgb);
}
fn puppy_edge_upload_highlight(a,b) {
    let dir=cg_normal(cg_sub(b,a));if !dir {return 0;}
    let ref=cg_v(0,0,1000000);
    if cg_abs(load64(dir+16))>900000000 {ref=cg_v(1000000,0,0);}
    let u=cg_normal(cg_cross(dir,ref));if !u {return 0;}
    let v=cg_normal(cg_cross(dir,cg_v(load64(u)/1000,load64(u+8)/1000,load64(u+16)/1000)));
    if !v {return 0;}
    let distance=puppy_radius*puppy_zoom/100+2;
    let h=gui_height(load64(puppy_device));if h<1 {h=640;}
    let rad=puppy_limit(distance*2000000/h,40000,2500000);
    let a0=puppy_edge_offset(a,u,rad);let a1=puppy_edge_offset(a,v,rad);
    let a2=puppy_edge_offset(a,u,-rad);let a3=puppy_edge_offset(a,v,-rad);
    let b0=puppy_edge_offset(b,u,rad);let b1=puppy_edge_offset(b,v,rad);
    let b2=puppy_edge_offset(b,u,-rad);let b3=puppy_edge_offset(b,v,-rad);
    let tube=m_buffer();
    puppy_edge_quad(tube,a0,a1,b1,b0,0xffa600);
    puppy_edge_quad(tube,a1,a2,b2,b1,0xffa600);
    puppy_edge_quad(tube,a2,a3,b3,b2,0xffa600);
    puppy_edge_quad(tube,a3,a0,b0,b3,0xffa600);
    if !puppy_face_tube_mesh {puppy_face_tube_mesh=puppy_colored_mesh();}
    if !render_mesh_upload(puppy_face_tube_mesh,m_data(tube),24,0x88e4) {return 0;}
    puppy_face_tube_count=24;
    puppy_sel_edge_key=(load64(a)+load64(b))*31+(load64(a+8)+load64(b+8))*131+(load64(a+16)+load64(b+16))*1009+1;
    if !puppy_sel_edge_key {puppy_sel_edge_key=1;}
    let line=m_buffer();
    puppy_face_vertex(line,a,0xffa600);puppy_face_vertex(line,b,0xffa600);
    if !puppy_face_edge_mesh {puppy_face_edge_mesh=puppy_colored_mesh();}
    if render_mesh_upload(puppy_face_edge_mesh,m_data(line),2,0x88e4) {puppy_face_edge_count=2;}
    return 1;
}
fn puppy_project_bodies(project) {
    let item=cad_project_item(project);
    if cad_project_is_studio(project) {return pcm_evaluate(j_parse(j_encode(project)),1);}
    if m_eq(j_string(item,"type"),"assembly") {return pc_assembly_bodies(project,item);}
    return pc_part_bodies(item);
}
fn puppy_edge_select(project,ray,hit) {
    if !ray {return 0;}
    cg_begin();
    let eye=load64(ray);let dir=pc_pick_direction(eye,load64(ray+8));
    if !dir {cg_error=0;return 0;}
    let distance=puppy_radius*puppy_zoom/100+2;
    let h=gui_height(load64(puppy_device));if h<1 {h=640;}
    let tol=puppy_limit(distance*5200000/h,10000,500000000);
    let limit=(tol/1000)*(tol/1000);
    let bodies=pc_visible_bodies(puppy_project_bodies(project),puppy_visible_id());
    if cg_error {cg_error=0;return 0;}
    let depth=20000001;let best=0;let best_body=0;let i=0;
    while i<cg_count(bodies) && !cg_error {
        let body=cg_at(bodies,i);let edges=pc_body_edges(load64(body+24));let j=0;
        while j<cg_count(edges) && !cg_error {
            let e=cg_at(edges,j);
            if load64(e+48) {
                let cand=pc_edge_ray_candidate(e,eye,dir,limit);
                if cand && (!best || load64(cand)<limit || load64(cand+8)<depth) {
                    let vhit=pc_visible_hit(bodies,load64(cand+16),dir);
                    if !vhit || cg_dot(dir,cg_sub(load64(vhit),load64(cand+16)))/1000+6>=load64(cand+8) {
                        best=e;best_body=body;limit=load64(cand);depth=load64(cand+8);
                    }
                }
            }
            j=j+1;
        }
        i=i+1;
    }
    if cg_error || !best {cg_error=0;return 0;}
    puppy_face_clear();
    if !puppy_edge_upload_highlight(load64(best),load64(best+8)) {return 0;}
    if cad_project_part(project) && load64(best+48) {
        let ref=j_object();j_add(ref,"a",pcd_json_vector(load64(best)));j_add(ref,"b",pcd_json_vector(load64(best+8)));
        let pair=j_array();j_add(ref,"normals",pair);i=0;
        while i<2 {
            let n=load64(best+16+i*8);
            j_add(pair,0,pcd_json_vector(cg_v(load64(n)/1000,load64(n+8)/1000,load64(n+16)/1000)));i=i+1;
        }
        puppy_selected_edge=puppy_snapshot(j_encode(ref));
    }
    return best_body;
}
fn puppy_hover_cache_skverts(paths) {
    puppy_hover_free_skverts();
    if !paths || !cg_count(paths) {return 1;}
    let pts=cg_array();let i=0;
    while i<cg_count(paths) && cg_count(pts)<512 {
        let path=cg_at(paths,i);let list=load64(path);let n=cg_count(list);
        if n<=8 {
            let j=0;while j<n && cg_count(pts)<512 {puppy_face_add_unique(pts,cg_at(list,j));j=j+1;}
        }else if n>1 {
            puppy_face_add_unique(pts,cg_at(list,0));
            puppy_face_add_unique(pts,cg_at(list,n-1));
        }
        i=i+1;
    }
    let count=cg_count(pts);if !count {return 1;}
    let bytes=count*24;let buf=alloc(bytes);if buf<=0 {return 0;}
    i=0;while i<count {m_copy(buf+i*24,cg_at(pts,i),24);i=i+1;}
    puppy_hover_skverts=buf;puppy_hover_skvert_count=count;puppy_hover_skvert_bytes=bytes;
    return 1;
}
fn puppy_uf_find(parent,x) {
    let r=x;
    while load64(parent+r*8)!=r {r=load64(parent+r*8);}
    while load64(parent+x*8)!=r {
        let next=load64(parent+x*8);store64(parent+x*8,r);x=next;
    }
    return r;
}
fn puppy_hover_edge_add(buckets,edges,ecount_ptr,a,b,part_idx,tri) {
    if a>b {let tmp=a;a=b;b=tmp;}
    let h=((a*73856093)^(b*19349663)^(part_idx*83492791))&4095;
    let idx=load64(buckets+h*8);
    while idx>=0 {
        let e=edges+idx*56;
        if load64(e)==a && load64(e+8)==b && load64(e+16)==part_idx {
            if load64(e+32)<0 {store64(e+32,tri);}
            else {store64(e+40,1);}
            return 1;
        }
        idx=load64(e+48);
    }
    let cur=load64(ecount_ptr);
    let e=edges+cur*56;
    store64(e,a);store64(e+8,b);store64(e+16,part_idx);
    store64(e+24,tri);store64(e+32,-1);store64(e+40,0);
    store64(e+48,load64(buckets+h*8));
    store64(buckets+h*8,cur);
    store64(ecount_ptr,cur+1);
    return 1;
}
fn puppy_hover_sharp_dir_add(sdeg,sdir0,sdir1,v,d,sign) {
    let deg=load64(sdeg+v*8);
    if deg<2 {
        let dst=sdir0+v*24;if deg==1 {dst=sdir1+v*24;}
        store64(dst,load64(d)*sign);
        store64(dst+8,load64(d+8)*sign);
        store64(dst+16,load64(d+16)*sign);
    }
    store64(sdeg+v*8,deg+1);return 1;
}
fn puppy_hover_cache_mesh(mesh,isolated) {
    puppy_hover_free_cache();
    let vlist=j_get(mesh,"vertices");let total=cg_j_count(vlist);
    if !total {return 1;}
    let cap=total;if cap>32768 {cap=32768;}
    let bytes=cap*24;let buf=alloc(bytes);if buf<=0 {return 0;}
    let p=pc_first(vlist);let i=0;let count=0;
    while p && count<cap {
        let c=pc_first(p);let k=0;
        while k<3 && c {
            store64(buf+count*24+k*8,cg_number(c));
            c=load64(c+24);k=k+1;
        }
        if !cg_error && k==3 {count=count+1;}else {cg_error=0;gui__zero(buf+count*24,24);count=count+1;}
        p=load64(p+24);i=i+1;
    }
    if !count {syscall(11,buf,bytes,0,0,0,0);return 1;}
    let cbytes=count;let cbuf=alloc(cbytes);
    if cbuf<=0 {syscall(11,buf,bytes,0,0,0,0);return 1;}
    gui__zero(cbuf,cbytes);
    puppy_hover_verts=buf;puppy_hover_vert_count=count;puppy_hover_vert_bytes=bytes;
    puppy_hover_vert_corner=cbuf;puppy_hover_vert_corner_bytes=cbytes;
    let total_tris=j_integer(mesh,"triangles");if !total_tris {return 1;}
    let tcap=total_tris;if tcap>32768 {tcap=32768;}
    let tbytes=tcap*72;let tbuf=alloc(tbytes);if tbuf<=0 {return 1;}
    let nbytes=tcap*36;let nbuf=alloc(nbytes);
    if nbuf<=0 {syscall(11,tbuf,tbytes,0,0,0,0);return 1;}
    gui__zero(nbuf,nbytes);
    let tcount=0;let raw_idx=0;let part_idx=0;let part=pc_first(j_get(mesh,"parts"));
    let vis_first=0;let vis_end=0;let vis_set=0;
    while part && raw_idx<tcap {
        let part_start=tcount;
        let rgb=puppy_part_rgb(part);let tag=(part_idx<<32)|(rgb&0xffffffff);
        let id=pc_first(j_get(part,"triangles"));
        while id && load64(id+24) && load64(load64(id+24)+24) && raw_idx<tcap {
            let i0=j_value(id);let id1=load64(id+24);let i1=j_value(id1);
            let id2=load64(id1+24);let i2=j_value(id2);
            id=load64(id2+24);
            if i0>=0 && i0<count && i1>=0 && i1<count && i2>=0 && i2<count {
                let v0=buf+i0*24;let v1=buf+i1*24;let v2=buf+i2*24;
                let e1=cg_v((load64(v1)-load64(v0))/1000,(load64(v1+8)-load64(v0+8))/1000,(load64(v1+16)-load64(v0+16))/1000);
                let e2=cg_v((load64(v2)-load64(v0))/1000,(load64(v2+8)-load64(v0+8))/1000,(load64(v2+16)-load64(v0+16))/1000);
                let n=cg_normal(cg_cross(e1,e2));
                if n {
                    let row=tbuf+tcount*72;
                    store64(row,i0);store64(row+8,i1);store64(row+16,i2);
                    store64(row+24,load64(n));store64(row+32,load64(n+8));store64(row+40,load64(n+16));
                    store64(row+48,tag);store64(row+56,tcount);store64(row+64,raw_idx);
                    tcount=tcount+1;
                }
            }
            raw_idx=raw_idx+1;
        }
        if isolated && part==isolated {vis_first=part_start;vis_end=tcount;vis_set=1;}
        part_idx=part_idx+1;part=load64(part+24);
    }
    if !tcount {
        syscall(11,tbuf,tbytes,0,0,0,0);syscall(11,nbuf,nbytes,0,0,0,0);return 1;
    }
    if !vis_set {vis_first=0;vis_end=tcount;}
    // Build O(T) undirected edge table to group smooth surfaces and detect sharp corners.
    let buckets=m_take(4096*8);i=0;while i<4096 {store64(buckets+i*8,-1);i=i+1;}
    let edges=m_take(tcount*3*56);let ecount_ptr=m_take(8);store64(ecount_ptr,0);
    i=0;while i<tcount {
        let row=tbuf+i*72;let pid=load64(row+48)>>32;
        let a=load64(row);let b=load64(row+8);let c=load64(row+16);
        puppy_hover_edge_add(buckets,edges,ecount_ptr,a,b,pid,i);
        puppy_hover_edge_add(buckets,edges,ecount_ptr,b,c,pid,i);
        puppy_hover_edge_add(buckets,edges,ecount_ptr,c,a,pid,i);
        i=i+1;
    }
    let ecount=load64(ecount_ptr);
    let cparent=m_take(tcount*8);i=0;while i<tcount {store64(cparent+i*8,i);i=i+1;}
    i=0;while i<ecount {
        let e=edges+i*56;let t0=load64(e+24);let t1=load64(e+32);
        if t1>=0 && !load64(e+40) {
            if 1000000000-cg_dot(tbuf+t0*72+24,tbuf+t1*72+24)<500000 {
                let ra=puppy_uf_find(cparent,t0);let rb=puppy_uf_find(cparent,t1);
                if ra!=rb {store64(cparent+rb*8,ra);}
            }
        }
        i=i+1;
    }
    let max_smooth=m_take(tcount*8);gui__zero(max_smooth,tcount*8);
    i=0;while i<ecount {
        let e=edges+i*56;let t0=load64(e+24);let t1=load64(e+32);
        if t1>=0 && !load64(e+40) {
            let dot=cg_dot(tbuf+t0*72+24,tbuf+t1*72+24);
            let d=1000000000-dot;
            if d>=500000 && dot>=939692621 {
                let c0=puppy_uf_find(cparent,t0);let c1=puppy_uf_find(cparent,t1);
                if d>load64(max_smooth+c0*8) {store64(max_smooth+c0*8,d);}
                if d>load64(max_smooth+c1*8) {store64(max_smooth+c1*8,d);}
            }
        }
        i=i+1;
    }
    let parent=m_take(tcount*8);i=0;while i<tcount {store64(parent+i*8,puppy_uf_find(cparent,i));i=i+1;}
    let sdeg=m_take(count*8);gui__zero(sdeg,count*8);
    let sdir0=m_take(count*24);let sdir1=m_take(count*24);
    i=0;while i<ecount {
        let e=edges+i*56;let a=load64(e);let b=load64(e+8);
        let t0=load64(e+24);let t1=load64(e+32);let multi=load64(e+40);
        let sharp=1;
        if t1>=0 && !multi {
            let dot=cg_dot(tbuf+t0*72+24,tbuf+t1*72+24);
            if dot>=939692621 {
                sharp=0;
                let d=1000000000-dot;
                let c0=puppy_uf_find(cparent,t0);let c1=puppy_uf_find(cparent,t1);
                // Keep tangent flat-to-fillet seams separate (half-step 1-cos is 4x smaller than strip step).
                if d>=500000 && d*5>=load64(max_smooth+c0*8)*2 && d*5>=load64(max_smooth+c1*8)*2 {
                    let ra=puppy_uf_find(parent,t0);let rb=puppy_uf_find(parent,t1);
                    if ra!=rb {store64(parent+rb*8,ra);}
                }
            }
        }
        if sharp && ((t0>=vis_first && t0<vis_end) || (t1>=vis_first && t1<vis_end)) {
            let dir=cg_normal(cg_sub(buf+b*24,buf+a*24));
            if dir {
                puppy_hover_sharp_dir_add(sdeg,sdir0,sdir1,a,dir,1);
                puppy_hover_sharp_dir_add(sdeg,sdir0,sdir1,b,dir,-1);
            }
        }
        i=i+1;
    }
    i=0;while i<tcount {
        store64(tbuf+i*72+56,puppy_uf_find(parent,i));
        i=i+1;
    }
    // Mark only genuine CAD corner vertices (excluding internal smooth/rim tessellation points).
    i=0;while i<count {
        let deg=load64(sdeg+i*8);let flag=0;
        if deg==1 || deg>=3 {flag=1;}
        else if deg==2 {
            if cg_dot(sdir0+i*24,sdir1+i*24)> -939692621 {flag=1;}
        }
        store8(cbuf+i,flag);
        i=i+1;
    }
    // Accumulate smooth per-(vertex, smooth-group) normals for curved surface shading.
    let vhead=m_take(count*8);i=0;while i<count {store64(vhead+i*8,-1);i=i+1;}
    let nodes=m_take(tcount*3*40);let ncount=0;
    i=0;while i<tcount {
        let row=tbuf+i*72;let grp=load64(row+56);
        let nx=load64(row+24)/1000;let ny=load64(row+32)/1000;let nz=load64(row+40)/1000;
        let c=0;while c<3 {
            let v=load64(row+c*8);
            let cur=load64(vhead+v*8);let found=-1;
            while cur>=0 && found<0 {
                let nd=nodes+cur*40;
                if load64(nd)==grp {found=cur;}
                else {cur=load64(nd+32);}
            }
            if found>=0 {
                let nd=nodes+found*40;
                store64(nd+8,load64(nd+8)+nx);
                store64(nd+16,load64(nd+16)+ny);
                store64(nd+24,load64(nd+24)+nz);
            }else {
                let nd=nodes+ncount*40;
                store64(nd,grp);store64(nd+8,nx);store64(nd+16,ny);store64(nd+24,nz);
                store64(nd+32,load64(vhead+v*8));
                store64(vhead+v*8,ncount);ncount=ncount+1;
            }
            c=c+1;
        }
        i=i+1;
    }
    i=0;while i<ncount {
        let nd=nodes+i*40;
        let u=cg_normal(cg_v(load64(nd+8),load64(nd+16),load64(nd+24)));
        if u {store64(nd+8,load64(u));store64(nd+16,load64(u+8));store64(nd+24,load64(u+16));}
        else {store64(nd+8,0);store64(nd+16,0);store64(nd+24,0);}
        i=i+1;
    }
    i=0;while i<tcount {
        let row=tbuf+i*72;let grp=load64(row+56);let ridx=load64(row+64);
        let c=0;while c<3 {
            let v=load64(row+c*8);let cur=load64(vhead+v*8);
            let dst=nbuf+ridx*36+c*12;
            while cur>=0 {
                let nd=nodes+cur*40;
                if load64(nd)==grp {
                    render_float(dst,load64(nd+8),1000000000);
                    render_float(dst+4,load64(nd+16),1000000000);
                    render_float(dst+8,load64(nd+24),1000000000);
                    cur=-1;
                }else {cur=load64(nd+32);}
            }
            c=c+1;
        }
        i=i+1;
    }
    puppy_hover_tris=tbuf;puppy_hover_tri_count=tcount;puppy_hover_tri_bytes=tbytes;
    puppy_hover_tri_norms=nbuf;puppy_hover_tri_norms_bytes=nbytes;puppy_hover_raw_tri_count=raw_idx;
    puppy_hover_vis_first=vis_first;puppy_hover_vis_end=vis_end;
    return 1;
}
fn puppy_hover_tri_hit(origin,dir) {
    if !puppy_hover_tri_count {return 0;}
    let nearest=20000001;let best=-1;let hit_p=m_take(24);
    let i=puppy_hover_vis_first;
    while i<puppy_hover_vis_end {
        let row=puppy_hover_tris+i*72;let n=row+24;
        let den=cg_dot(n,dir);
        if den< -10000 {
            let v0=puppy_hover_verts+load64(row)*24;
            let dot=((load64(v0)-load64(origin))/1000)*load64(n)
                   +((load64(v0+8)-load64(origin+8))/1000)*load64(n+8)
                   +((load64(v0+16)-load64(origin+16))/1000)*load64(n+16);
            let dist=dot/den;
            if dist>=0 && dist<nearest {
                let px=load64(origin)+dist*load64(dir)/1000000;
                let py=load64(origin+8)+dist*load64(dir+8)/1000000;
                let pz=load64(origin+16)+dist*load64(dir+16)/1000000;
                let major=0;if cg_abs(load64(n+8))>cg_abs(load64(n)) {major=8;}
                if cg_abs(load64(n+16))>cg_abs(load64(n+major)) {major=16;}
                let u=(major+8)%24;let v=(major+16)%24;
                let pts=cg_v(v0,puppy_hover_verts+load64(row+8)*24,puppy_hover_verts+load64(row+16)*24);
                let pt=cg_v(px,py,pz);let inside=1;let e=0;
                while e<3 && inside {
                    let a=load64(pts+e*8);let b=load64(pts+((e+1)%3)*8);
                    let dx=(load64(b+u)-load64(a+u))/1000;let dy=(load64(b+v)-load64(a+v))/1000;
                    let turn=dx*((load64(pt+v)-load64(a+v))/1000)-dy*((load64(pt+u)-load64(a+u))/1000);
                    if load64(n+major)<0 {turn=-turn;}
                    if turn< -4*(cg_abs(dx)+cg_abs(dy)) {inside=0;}
                    e=e+1;
                }
                if inside {
                    nearest=dist;best=i;
                    store64(hit_p,px);store64(hit_p+8,py);store64(hit_p+16,pz);
                }
            }
        }
        i=i+1;
    }
    if best<0 {return 0;}
    return cg_v(best,nearest,hit_p);
}
fn puppy_hover_vertex_visible(v,origin,dir) {
    if !puppy_hover_tri_count {return 1;}
    let vorigin=origin;let vdir=0;let v_along=0;
    if puppy_orthographic {
        let shift=(puppy_radius*puppy_zoom/100+puppy_radius*2+10)*1000;
        vdir=dir;v_along=shift;
        vorigin=cg_v(load64(v)-shift*load64(dir)/1000000,
                     load64(v+8)-shift*load64(dir+8)/1000000,
                     load64(v+16)-shift*load64(dir+16)/1000000);
    }else {
        vdir=cg_normal(cg_sub(v,origin));if !vdir {return 1;}
        v_along=cg_dot(vdir,cg_sub(v,origin))/1000;
    }
    let hit=puppy_hover_tri_hit(vorigin,vdir);
    if !hit {return 1;}
    return load64(hit+8)+350>=v_along;
}
fn puppy_hover_offset(p,axis,rad) {
    return cg_v(load64(p)+load64(axis)*rad/1000000,
                load64(p+8)+load64(axis+8)*rad/1000000,
                load64(p+16)+load64(axis+16)*rad/1000000);
}
fn puppy_vertex_coords_text() {
    return m_cat(puppy_display_number(puppy_sel_vx),m_cat(", ",m_cat(puppy_display_number(puppy_sel_vy),m_cat(", ",m_cat(puppy_display_number(puppy_sel_vz)," mm")))));
}
fn puppy_vertex_badge_upload(p,rad_scale,selected) {
    let distance=puppy_radius*puppy_zoom/100+2;
    let h=gui_height(load64(puppy_device));if h<1 {h=640;}
    let rad=puppy_limit(distance*rad_scale/h,80000,6000000);
    let inner=rad*45/100;if inner<30000 {inner=30000;}
    let right=puppy_cam_basis;let up=puppy_cam_basis+24;let forward=puppy_cam_basis+48;
    let c0=puppy_hover_offset(puppy_hover_offset(p,right,-rad),up,-rad);
    let c1=puppy_hover_offset(puppy_hover_offset(p,right,rad),up,-rad);
    let c2=puppy_hover_offset(puppy_hover_offset(p,right,rad),up,rad);
    let c3=puppy_hover_offset(puppy_hover_offset(p,right,-rad),up,rad);
    let front=puppy_hover_offset(p,forward,-rad/3);
    let d0=puppy_hover_offset(puppy_hover_offset(front,right,-inner),up,-inner);
    let d1=puppy_hover_offset(puppy_hover_offset(front,right,inner),up,-inner);
    let d2=puppy_hover_offset(puppy_hover_offset(front,right,inner),up,inner);
    let d3=puppy_hover_offset(puppy_hover_offset(front,right,-inner),up,inner);
    let buf=m_buffer();
    puppy_edge_quad(buf,c0,c1,c2,c3,0xffa600);
    puppy_edge_quad(buf,d0,d1,d2,d3,0xfff3d1);
    if selected {
        if !puppy_sel_vert_mesh {puppy_sel_vert_mesh=puppy_colored_mesh();}
        if !render_mesh_upload(puppy_sel_vert_mesh,m_data(buf),12,0x88e8) {return 0;}
        puppy_sel_vert_count=12;return 1;
    }
    if !puppy_hover_mesh {puppy_hover_mesh=puppy_colored_mesh();}
    if !render_mesh_upload(puppy_hover_mesh,m_data(buf),12,0x88e8) {return 0;}
    puppy_hover_count=12;return 1;
}
fn puppy_find_vertex_in(verts,count,origin,dir,limit2,check_vis) {
    let best=-1;let best_along=0x7fffffffffffffff;let best_dist2=limit2+1;
    let i=0;
    while i<count {
        if !check_vis || !puppy_hover_vert_corner || load8(puppy_hover_vert_corner+i) {
            let v=verts+i*24;
            let ox=(load64(v)-load64(origin))/1000;
            let oy=(load64(v+8)-load64(origin+8))/1000;
            let oz=(load64(v+16)-load64(origin+16))/1000;
            let along=(ox*load64(dir)+oy*load64(dir+8)+oz*load64(dir+16))/1000000000;
            if along>0 {
                let mx=ox-along*load64(dir)/1000000000;
                let my=oy-along*load64(dir+8)/1000000000;
                let mz=oz-along*load64(dir+16)/1000000000;
                let d2=mx*mx+my*my+mz*mz;
                if d2<=limit2 && (best<0 || along+50<best_along || (along<=best_along+50 && d2<best_dist2)) {
                    if !check_vis || puppy_hover_vertex_visible(v,origin,dir) {
                        best=i;best_along=along;best_dist2=d2;
                    }
                }
            }
        }
        i=i+1;
    }
    return best;
}
fn puppy_find_visible_vertex(x,y,tol_scale,check_vis) {
    if !puppy_hover_vert_count && !puppy_hover_skvert_count {return -1;}
    let h=gui_height(load64(puppy_device));if h<1 {return -1;}
    let ray=puppy_ray(x,y);if !ray {cg_error=0;return -1;}
    let origin=load64(ray);let dir=cg_normal(load64(ray+8));if !dir {cg_error=0;return -1;}
    let distance=puppy_radius*puppy_zoom/100+2;
    let tol=puppy_limit(distance*tol_scale/h,10,500000);let limit2=tol*tol;
    let best=puppy_find_vertex_in(puppy_hover_verts,puppy_hover_vert_count,origin,dir,limit2,check_vis);
    if best>=0 {return best;}
    if puppy_part_sketch_available() && puppy_hover_skvert_count {
        let sb=puppy_find_vertex_in(puppy_hover_skverts,puppy_hover_skvert_count,origin,dir,limit2,0);
        if sb>=0 {return 100000+sb;}
    }
    return -1;
}
fn puppy_hover_vertex_ptr(idx) {
    if idx>=100000 {return puppy_hover_skverts+(idx-100000)*24;}
    return puppy_hover_verts+idx*24;
}
fn puppy_vertex_select(x,y) {
    let idx=puppy_find_visible_vertex(x,y,6000,1);
    if idx<0 {return 0;}
    let v=puppy_hover_vertex_ptr(idx);
    puppy_face_clear();
    puppy_selected_vertex=1;
    puppy_sel_vx=load64(v);puppy_sel_vy=load64(v+8);puppy_sel_vz=load64(v+16);
    puppy_vertex_badge_upload(v,5200000,1);
    return 1;
}
fn puppy_hover_blend_tint(rgb) {
    let r=((rgb>>16)&255)*55/100+255*45/100;
    let g=((rgb>>8)&255)*55/100+176*45/100;
    let b=(rgb&255)*55/100+32*45/100;
    return (r<<16)|(g<<8)|b;
}
fn puppy_hover_tri_touch(ra,rb) {
    let a=cg_v(load64(ra),load64(ra+8),load64(ra+16));
    let b=cg_v(load64(rb),load64(rb+8),load64(rb+16));
    let i=0;while i<3 {
        let pa=puppy_hover_verts+load64(a+i*8)*24;let j=0;
        while j<3 {
            if load64(a+i*8)==load64(b+j*8) {return 1;}
            let pb=puppy_hover_verts+load64(b+j*8)*24;
            if puppy_face_point_near(pa,pb) {return 1;}
            j=j+1;
        }
        j=0;while j<3 {
            let pb0=puppy_hover_verts+load64(b+j*8)*24;
            let pb1=puppy_hover_verts+load64(b+((j+1)%3)*8)*24;
            if puppy_face_point_on_seg(pa,pb0,pb1)>0 {return 1;}
            j=j+1;
        }
        i=i+1;
    }
    let j=0;while j<3 {
        let pb=puppy_hover_verts+load64(b+j*8)*24;let k=0;
        while k<3 {
            let pa0=puppy_hover_verts+load64(a+k*8)*24;
            let pa1=puppy_hover_verts+load64(a+((k+1)%3)*8)*24;
            if puppy_face_point_on_seg(pb,pa0,pa1)>0 {return 1;}
            k=k+1;
        }
        j=j+1;
    }
    return 0;
}
fn puppy_hover_same_face(a_idx,b_idx) {
    if a_idx<puppy_hover_vis_first || b_idx<puppy_hover_vis_first || a_idx>=puppy_hover_vis_end || b_idx>=puppy_hover_vis_end {return 0;}
    if a_idx==b_idx {return 1;}
    let ra=puppy_hover_tris+a_idx*72;let rb=puppy_hover_tris+b_idx*72;
    if load64(ra+56)==load64(rb+56) {return 1;}
    if load64(ra+48)!=load64(rb+48) {return 0;}
    if cg_dot(ra+24,rb+24)<=999000000 {return 0;}
    let va=puppy_hover_verts+load64(ra)*24;let vb=puppy_hover_verts+load64(rb)*24;
    return cg_abs(cg_dot(ra+24,cg_sub(vb,va)))<10000;
}
fn puppy_surface_upload_from_tri(seed_idx,is_select) {
    let seed=puppy_hover_tris+seed_idx*72;let tag=load64(seed+48);let grp=load64(seed+56);
    let sn=seed+24;let sv0=puppy_hover_verts+load64(seed)*24;
    let vis_n=puppy_hover_vis_end-puppy_hover_vis_first;if vis_n<1 {return 0;}
    let sel=m_take(vis_n*8);let scount=0;let planar=1;
    let i=puppy_hover_vis_first;
    while i<puppy_hover_vis_end {
        let row=puppy_hover_tris+i*72;
        if load64(row+56)==grp {
            store64(sel+scount*8,i);scount=scount+1;
            if cg_dot(row+24,sn)<=999000000 {planar=0;}
        }
        i=i+1;
    }
    if !scount {return 0;}
    if planar {
        // Merge coplanar T-junction patches that did not share an exact vertex-index edge.
        let plane=m_take(vis_n*8);let pcount=0;let seed_slot=0;
        i=puppy_hover_vis_first;
        while i<puppy_hover_vis_end {
            let row=puppy_hover_tris+i*72;
            if load64(row+48)==tag && cg_dot(row+24,sn)>999000000 {
                let v0=puppy_hover_verts+load64(row)*24;
                if cg_abs(cg_dot(sn,cg_sub(v0,sv0)))<10000 {
                    if i==seed_idx {seed_slot=pcount;}
                    store64(plane+pcount*8,i);pcount=pcount+1;
                }
            }
            i=i+1;
        }
        if pcount>scount {
            let marks=m_take(pcount*8);gui__zero(marks,pcount*8);
            let queue=m_take(pcount*8);store64(marks+seed_slot*8,1);store64(queue,seed_slot);
            let head=0;let tail=1;
            while head<tail {
                let cur=puppy_hover_tris+load64(plane+load64(queue+head*8)*8)*72;let v=0;
                while v<pcount {
                    if !load64(marks+v*8) && puppy_hover_tri_touch(cur,puppy_hover_tris+load64(plane+v*8)*72) {
                        store64(marks+v*8,1);store64(queue+tail*8,v);tail=tail+1;
                    }
                    v=v+1;
                }
                head=head+1;
            }
            scount=tail;i=0;
            while i<scount {store64(sel+i*8,load64(plane+load64(queue+i*8)*8));i=i+1;}
        }
    }
    let fill_rgb=puppy_hover_blend_tint(tag&0xffffffff);
    let edge_rgb=0xffb833;
    if is_select {fill_rgb=0xd48e18;edge_rgb=0xffa600;}
    let fill_buf=m_buffer();let edge_buf=m_buffer();
    if planar {
        let all_pts=cg_array();i=0;
        while i<scount {
            let row=puppy_hover_tris+load64(sel+i*8)*72;let ridx=load64(row+64);
            let p0=puppy_hover_verts+load64(row)*24;
            let p1=puppy_hover_verts+load64(row+8)*24;
            let p2=puppy_hover_verts+load64(row+16)*24;
            puppy_face_add_unique(all_pts,p0);puppy_face_add_unique(all_pts,p1);puppy_face_add_unique(all_pts,p2);
            let nb=0;let n1=0;let n2=0;
            if puppy_hover_tri_norms && ridx<puppy_hover_raw_tri_count {nb=puppy_hover_tri_norms+ridx*36;n1=nb+12;n2=nb+24;}
            puppy_shaded_vertex(fill_buf,p0,fill_rgb,nb);
            puppy_shaded_vertex(fill_buf,p1,fill_rgb,n1);
            puppy_shaded_vertex(fill_buf,p2,fill_rgb,n2);
            i=i+1;
        }
        let segs=cg_array();i=0;
        while i<scount {
            let row=puppy_hover_tris+load64(sel+i*8)*72;
            let p0=puppy_hover_verts+load64(row)*24;
            let p1=puppy_hover_verts+load64(row+8)*24;
            let p2=puppy_hover_verts+load64(row+16)*24;
            puppy_face_emit_split_edge(segs,all_pts,p0,p1);
            puppy_face_emit_split_edge(segs,all_pts,p1,p2);
            puppy_face_emit_split_edge(segs,all_pts,p2,p0);
            i=i+1;
        }
        let nsegs=cg_count(segs);let cancelled=m_take((nsegs+1)*8);gui__zero(cancelled,(nsegs+1)*8);
        i=0;while i<nsegs {
            if !load64(cancelled+i*8) {
                let si=cg_at(segs,i);let ui=load64(si);let vi=load64(si+8);let j=i+1;
                while j<nsegs {
                    if !load64(cancelled+j*8) {
                        let sj=cg_at(segs,j);
                        if puppy_face_point_near(ui,load64(sj+8)) && puppy_face_point_near(vi,load64(sj)) {
                            store64(cancelled+i*8,1);store64(cancelled+j*8,1);j=nsegs;
                        }
                    }
                    j=j+1;
                }
            }
            i=i+1;
        }
        i=0;while i<nsegs {
            if !load64(cancelled+i*8) {
                let s=cg_at(segs,i);
                puppy_face_vertex(edge_buf,load64(s),edge_rgb);
                puppy_face_vertex(edge_buf,load64(s+8),edge_rgb);
            }
            i=i+1;
        }
    }else {
        let eslot=m_take(scount*3*16);let nsegs=0;i=0;
        while i<scount {
            let row=puppy_hover_tris+load64(sel+i*8)*72;let ridx=load64(row+64);
            let i0=load64(row);let i1=load64(row+8);let i2=load64(row+16);
            let p0=puppy_hover_verts+i0*24;let p1=puppy_hover_verts+i1*24;let p2=puppy_hover_verts+i2*24;
            let nb=0;let n1=0;let n2=0;
            if puppy_hover_tri_norms && ridx<puppy_hover_raw_tri_count {nb=puppy_hover_tri_norms+ridx*36;n1=nb+12;n2=nb+24;}
            puppy_shaded_vertex(fill_buf,p0,fill_rgb,nb);
            puppy_shaded_vertex(fill_buf,p1,fill_rgb,n1);
            puppy_shaded_vertex(fill_buf,p2,fill_rgb,n2);
            store64(eslot+nsegs*16,i0);store64(eslot+nsegs*16+8,i1);nsegs=nsegs+1;
            store64(eslot+nsegs*16,i1);store64(eslot+nsegs*16+8,i2);nsegs=nsegs+1;
            store64(eslot+nsegs*16,i2);store64(eslot+nsegs*16+8,i0);nsegs=nsegs+1;
            i=i+1;
        }
        let cancelled=m_take((nsegs+1)*8);gui__zero(cancelled,(nsegs+1)*8);
        i=0;while i<nsegs {
            if !load64(cancelled+i*8) {
                let ui=load64(eslot+i*16);let vi=load64(eslot+i*16+8);
                let pui=puppy_hover_verts+ui*24;let pvi=puppy_hover_verts+vi*24;
                let j=i+1;
                while j<nsegs {
                    if !load64(cancelled+j*8) {
                        let uj=load64(eslot+j*16);let vj=load64(eslot+j*16+8);
                        if (ui==vj && vi==uj) || (puppy_face_point_near(pui,puppy_hover_verts+vj*24) && puppy_face_point_near(pvi,puppy_hover_verts+uj*24)) {
                            store64(cancelled+i*8,1);store64(cancelled+j*8,1);j=nsegs;
                        }
                    }
                    j=j+1;
                }
            }
            i=i+1;
        }
        i=0;while i<nsegs {
            if !load64(cancelled+i*8) {
                puppy_face_vertex(edge_buf,puppy_hover_verts+load64(eslot+i*16)*24,edge_rgb);
                puppy_face_vertex(edge_buf,puppy_hover_verts+load64(eslot+i*16+8)*24,edge_rgb);
            }
            i=i+1;
        }
    }
    let fill_verts=load64(fill_buf+8)/28;let edge_verts=load64(edge_buf+8)/16;
    if !fill_verts {return 0;}
    if is_select {
        if !puppy_face_fill_mesh {puppy_face_fill_mesh=puppy_shaded_mesh();}
        if !render_mesh_upload(puppy_face_fill_mesh,m_data(fill_buf),fill_verts,0x88e4) {return 0;}
        puppy_face_fill_count=fill_verts;
        puppy_sel_face_key=puppy_sel_mesh_key(m_data(fill_buf),fill_verts,28);
        puppy_face_edge_count=0;
        if edge_verts {
            if !puppy_face_edge_mesh {puppy_face_edge_mesh=puppy_colored_mesh();}
            if render_mesh_upload(puppy_face_edge_mesh,m_data(edge_buf),edge_verts,0x88e4) {
                puppy_face_edge_count=edge_verts;
            }
        }
    }else {
        if !puppy_hover_face_fill_mesh {puppy_hover_face_fill_mesh=puppy_shaded_mesh();}
        if !render_mesh_upload(puppy_hover_face_fill_mesh,m_data(fill_buf),fill_verts,0x88e8) {return 0;}
        puppy_hover_face_fill_count=fill_verts;
        puppy_hover_face_edge_count=0;
        if edge_verts {
            if !puppy_hover_face_edge_mesh {puppy_hover_face_edge_mesh=puppy_colored_mesh();}
            if render_mesh_upload(puppy_hover_face_edge_mesh,m_data(edge_buf),edge_verts,0x88e8) {
                puppy_hover_face_edge_count=edge_verts;
            }
        }
    }
    if planar {return 2;}
    return 1;
}
fn puppy_hover_build_face(seed_idx) {
    return puppy_surface_upload_from_tri(seed_idx,0)>0;
}
fn puppy_hover_3d_move(x,y) {
    if (!puppy_hover_vert_count && !puppy_hover_skvert_count) || puppy_button || puppy_sketch
        || puppy_extrusion_view_active || puppy_reader_report || puppy_sheet_scene {
        return puppy_hover_clear();
    }
    let w=gui_width(load64(puppy_device));let h=gui_height(load64(puppy_device));
    if y<76 || y>=h || x<0 || x>=w {return puppy_hover_clear();}
    if puppy_design_open && puppy_design_available() && x<puppy_design_width() {return puppy_hover_clear();}
    if puppy_tree_open && x<puppy_objects_width() {return puppy_hover_clear();}
    if x>=w-104 && y<204 {return puppy_hover_clear();}
    let best=puppy_find_visible_vertex(x,y,9500,1);
    if best>=0 {
        if puppy_hover_kind==1 && best==puppy_hover_index {return 0;}
        puppy_hover_kind=1;puppy_hover_index=best;puppy_hover_tri_seed=-1;
        puppy_hover_face_fill_count=0;puppy_hover_face_edge_count=0;
        puppy_vertex_badge_upload(puppy_hover_vertex_ptr(best),4600000,0);
        return 1;
    }
    if puppy_empty || puppy_edge_kind || !puppy_hover_tri_count {return puppy_hover_clear();}
    let ray=puppy_ray(x,y);if !ray {cg_error=0;return puppy_hover_clear();}
    let origin=load64(ray);let dir=cg_normal(load64(ray+8));if !dir {cg_error=0;return puppy_hover_clear();}
    let hit=puppy_hover_tri_hit(origin,dir);
    if !hit {return puppy_hover_clear();}
    let tri=load64(hit);
    if puppy_hover_kind==2 && puppy_hover_same_face(tri,puppy_hover_tri_seed) {return 0;}
    puppy_hover_kind=2;puppy_hover_index=-1;puppy_hover_count=0;puppy_hover_tri_seed=tri;
    if !puppy_hover_build_face(tri) {return puppy_hover_clear();}
    return 1;
}
fn puppy_draw_face_selection() {
    if !puppy_face_fill_count && !puppy_face_tube_count && !puppy_face_edge_count
        && !puppy_hover_count && !puppy_sel_vert_count
        && !puppy_hover_face_fill_count && !puppy_hover_face_edge_count {return 1;}
    if puppy_draw_ranges && puppy_angle!=puppy_home {return 1;}
    if puppy_hover_face_fill_count && !puppy_draw_wire(puppy_hover_face_fill_mesh,3,4,0,puppy_hover_face_fill_count) {return 0;}
    if puppy_face_fill_count && !puppy_draw_wire(puppy_face_fill_mesh,3,4,0,puppy_face_fill_count) {return 0;}
    if puppy_hover_face_edge_count && !puppy_draw_wire(puppy_hover_face_edge_mesh,2,1,0,puppy_hover_face_edge_count) {return 0;}
    if puppy_face_tube_count && !puppy_draw_wire(puppy_face_tube_mesh,2,4,0,puppy_face_tube_count) {return 0;}
    if puppy_face_edge_count && !puppy_draw_wire(puppy_face_edge_mesh,2,1,0,puppy_face_edge_count) {return 0;}
    if puppy_sel_vert_count && !puppy_draw_wire(puppy_sel_vert_mesh,2,4,0,puppy_sel_vert_count) {return 0;}
    if puppy_hover_count && !puppy_draw_wire(puppy_hover_mesh,2,4,0,puppy_hover_count) {return 0;}
    return 1;
}
