global puppy_sketch=0;
global puppy_sketch_program=0;
global puppy_sketch_mesh=0;

fn puppy_sketch_open() {
    let vertex="#version 450 core\nlayout(location=0) in vec3 point; out float distancePx; uniform int guide; void main(){gl_Position=vec4(point.xy,0.0,1.0);distancePx=point.z;gl_PointSize=guide==5?13.0:7.0;}";
    let fragment="#version 450 core\nuniform int guide;in float distancePx;out vec4 color;void main(){if(guide==6 && mod(distancePx,12.0)>8.0)discard;if(guide==5){float d=length(gl_PointCoord-vec2(0.5));if(d>0.5)discard;color=d>0.32?vec4(0.87,0.94,0.97,1.0):vec4(0.22,0.58,0.74,1.0);return;}if(guide==3 && all(lessThan(abs(gl_PointCoord-vec2(0.5)),vec2(0.24)))) discard; color=(guide==4 || guide==6)?vec4(0.22,0.58,0.74,1.0):guide==2?vec4(1.0,0.69,0.27,1.0):guide==1?vec4(0.20,0.28,0.30,1.0):vec4(0.56,0.85,0.72,1.0);}";
    puppy_sketch_program=render_program(puppy_device,vertex,fragment);
    let attribute=m_take(48);gui__zero(attribute,48);
    store64(attribute+8,3);store64(attribute+16,0x1406);
    puppy_sketch_mesh=render_mesh(puppy_device,12,attribute,1);
    return puppy_sketch_program && puppy_sketch_mesh;
}
fn puppy_draw_sketch(w,h) {
    let project=cad_project_open(puppy_history+24);
    let region=puppy_profile(project,puppy_feature(project));
    if !region {return 0;}
    // Holes are strictly inside the outer loop, so its bounds frame the region.
    let outer=load64(cg_at(region,0));let first=cg_at(outer,0);
    let xmin=load64(first);let xmax=xmin;let ymin=load64(first+8);let ymax=ymin;let i=1;
    while i<cg_count(outer) {
        let point=cg_at(outer,i);let x=load64(point);let y=load64(point+8);
        if x<xmin {xmin=x;}if x>xmax {xmax=x;}if y<ymin {ymin=y;}if y>ymax {ymax=y;}
        i=i+1;
    }
    let width=xmax-xmin;let height=ymax-ymin;
    if width*h>height*w {height=width*h/w;}else {width=height*w/h;}
    if !width || !height {return 0;}
    render_gl(puppy_device,"glUseProgram",puppy_sketch_program,0,0,0,0,0);
    let guide=render_gl(puppy_device,"glGetUniformLocation",puppy_sketch_program,"guide",0,0,0,0);
    render_gl(puppy_device,"glUniform1i",guide,0,0,0,0,0);
    render_gl(puppy_device,"glDisable",0x0b71,0,0,0,0,0);
    i=0;
    while i<cg_count(region) {
        let points=load64(cg_at(region,i));let count=cg_count(points);let data=m_take(count*12);let j=0;
        while j<count {
            let p=cg_at(points,j);
            render_float(data+j*12,(2*load64(p)-xmin-xmax)*7,width*10);
            render_float(data+j*12+4,(2*load64(p+8)-ymin-ymax)*7,height*10);
            render_float(data+j*12+8,0,1);j=j+1;
        }
        if !render_mesh_upload(puppy_sketch_mesh,data,count,0x88e8)
            || !render_draw(puppy_device,load64(puppy_sketch_mesh+8),2,0,count) {return 0;}
        i=i+1;
    }
    return 1;
}
