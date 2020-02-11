///    ___           __________________  ___________
///   / _/__  ____  / __/ ___/  _/ __/ |/ / ___/ __/
///  / _/ _ \/ __/ _\ \/ /___/ // _//    / /__/ _/  
/// /_/ \___/_/   /___/\___/___/___/_/|_/\___/___/
///
/// Author : Benjamin Blundell - me@benjamin.computer
/// A small program that lets us view a directory of
/// tiff or fits files. It performs flattening and 
/// scaling so that we can view floating point images
/// in GTK which takes only RGB-8 images.
///
/// Using a little gtk-rs
/// https://gtk-rs.org/docs-src/tutorial/
/// And some fitrs
/// https://docs.rs/fitrs/0.5.0/fitrs/
///

extern crate image;
extern crate scoped_threadpool;
extern crate gtk;
extern crate gio;
extern crate gdk_pixbuf;
extern crate glib;
extern crate tiff;
extern crate fitrs;

use gtk::prelude::*;
use gio::prelude::*;
use gdk_pixbuf::Pixbuf;
use gdk_pixbuf::Colorspace;
use glib::Bytes;

use std::env;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{cell::Cell, rc::Rc, cell::RefCell};
use tiff::decoder::{Decoder, DecodingResult};
use tiff::ColorType;
use std::process;
use gtk::{Application, ApplicationWindow, Button, Label};
use fitrs::{Fits, FitsData, FitsDataArray};

// Holds our models and our GTK+ application
pub struct Explorer {
    app: gtk::Application,
    image_paths : Vec<PathBuf>,
    image_index : Cell<usize>, // use this so we can mutate it later
}

// Open a fits image, returing a gtk::Image and the width and height
fn get_image_fits(path : &Path ) -> (gtk::Image, usize, usize, f32, f32) {
    let fits = Fits::open(path).expect("Failed to open fits.");
    let mut img_buffer : Vec<Vec<f32>> = vec![];
    let mut width : usize = 0;
    let mut height : usize = 0;

    // Iterate over HDUs
    for hdu in fits.iter() {
        println!("{:?}", hdu.value("EXTNAME"));
        //println!("{:?}", hdu.read_data());
    }

    // Assume first hdu is the one we want. Won't be always
    // Get HDU by ID
    let hdu_0 = fits.get(0).unwrap();

    match hdu_0.read_data() {
        FitsData::FloatingPoint32(FitsDataArray { shape, data }) => {
            width = shape[1];
            height = shape[0];

            for _y in 0..height {
                let mut row  : Vec<f32> = vec![];
                for _x in 0..width {
                    row.push(0 as f32);
                }
                img_buffer.push(row);
            }

            for y in 0..height as usize {
                for x in 0..width as usize {
                    img_buffer[y][x] = data[y * height + x] as f32;
                }
            }
        }
        _ => { /* ... */ }
    }
    
    // Final buffer that we use that is a little smaller - u8
    // and not u16, but also RGB, just to make GTK happy.
    let mut final_buffer : Vec<u8> = vec![];
    for _y in 0..height {
        for _x in 0..width {
            // GTK insists we have RGB so we triple everything :/
            for _ in 0..3 {
                final_buffer.push(0 as u8);
            }
        }
    }

    // Find min/max
    let mut minp : f32 = 1e12; // we might end up overflowing!
    let mut maxp : f32 = 0.0;
    for y in 0..height {
        for x in 0..width {
            if (img_buffer[y][x] as f32) > maxp { maxp = img_buffer[y][x] as f32; }
            if (img_buffer[y][x] as f32) < minp { minp = img_buffer[y][x] as f32; }
        }
    }

    for y in 0..height {
        for x in 0..width  {
            let colour = (img_buffer[y][x] / maxp * 255.0) as u8;
            let idx = (y * (height ) + x) * 3;
            final_buffer[idx] = colour;
            final_buffer[idx+1] = colour;
            final_buffer[idx+2] = colour;
        }
    } 
   
    let b = Bytes::from(&final_buffer);

    let pixybuf = Pixbuf::new_from_bytes(&b,
        Colorspace::Rgb,
        false, 
        8,
        width as i32,
        height as i32,
        (width * 3 * 1) as i32
    );

    let image : gtk::Image = gtk::Image::new_from_pixbuf(Some(&pixybuf));
    return (image, width, height, minp, maxp);
}

// Convert our model into a gtk::Image that we can present to
// the screen.
fn get_image_tiff(path : &Path) -> (gtk::Image, usize, usize, f32, f32) {
    let img_file = File::open(path).expect("Cannot find test image!");
    let mut decoder = Decoder::new(img_file).expect("Cannot create decoder");

    let width : usize = decoder.dimensions().unwrap().0 as usize;
    let height : usize = decoder.dimensions().unwrap().1 as usize;

    assert_eq!(decoder.colortype().unwrap(), ColorType::Gray(16));
    let img_res = decoder.read_image().unwrap();

    // Our buffer - we sum all the image here and then scale
    let mut img_buffer : Vec<Vec<f32>> = vec![];
    for _y in 0..height {
        let mut row  : Vec<f32> = vec![];
        for _x in 0..width {
            row.push(0 as f32);
        }
        img_buffer.push(row);
    }
    
    // Final buffer that we use that is a little smaller - u8
    // and not u16, but also RGB, just to make GTK happy.
    let mut final_buffer : Vec<u8> = vec![];
    for _y in 0..height {
        for _x in 0..width {
            // GTK insists we have RGB so we triple everything :/
            for _ in 0..3 {
                final_buffer.push(0 as u8);
            }
        }
    }
   
    // Now we've decoded, lets update the img_buffer
    if let DecodingResult::U16(img_res) = img_res {
        let mut levels : usize = 0;
        for y in 0..height {
            for x in 0..width {
                img_buffer[y][x] = img_res[y * (height) + x] as f32;
            }
        }

        while decoder.more_images() {
            let next_res = decoder.next_image();
            match next_res {
                Ok(_res) => {   
                    let img_next = decoder.read_image().unwrap();
                    if let DecodingResult::U16(img_next) = img_next {
                        levels += 1;
                        for y in 0..height {
                            for x in 0..width {
                                img_buffer[y][x] += img_next[y * (height) + x] as f32;
                            }
                        } 
                    }
                },
                Err(_) => {}
            }
        }
        // We take an average rather than a total sum
        for y in 0..height {
            for x in 0..width {
                img_buffer[y][x] = img_buffer[y][x] / (levels as f32);
            }
        }

        // Find min/max
        let mut minp : f32 = 1e12; // we might end up overflowing!
        let mut maxp : f32 = 0.0;
        for y in 0..height {
            for x in 0..width {
                if (img_buffer[y][x] as f32) > maxp { maxp = img_buffer[y][x] as f32; }
                if (img_buffer[y][x] as f32) < minp { minp = img_buffer[y][x] as f32; }
            }
        }

        for y in 0..height {
            for x in 0..width {
                let colour = (img_buffer[y][x] / maxp * 255.0) as u8;
                let idx = (y * (height) + x) * 3;
                final_buffer[idx] = colour;
                final_buffer[idx+1] = colour;
                final_buffer[idx+2] = colour;
            }
        } 

        let b = Bytes::from(&final_buffer);
        println!("Succesfully read {} which has {} levels.", path.display(), levels);

        // Convert down the tiff so we can see it.
        
        let pixybuf = Pixbuf::new_from_bytes(&b,
            Colorspace::Rgb,
            false, 
            8,
            width as i32,
            height as i32,
            (width * 3 * 1) as i32
        );

        let image : gtk::Image = gtk::Image::new_from_pixbuf(Some(&pixybuf));
        return (image, width, height, minp, maxp);

    } else {
        panic!("Wrong data type");
    }

    let image: gtk::Image = gtk::Image::new();
    (image, width, height, 0.0, 0.0)
}

// Basic naive buffer copying program.
pub fn copy_buffer(in_buff : &Vec<Vec<f32>>, out_buff : &mut Vec<Vec<f32>>,
    width : usize, height : usize) {
    for _y in 0..height {
        for _x in 0..width {
            out_buff[_y][_x] = in_buff[_y][_x];
        }
    }
}

// Wrapper around the get_image_*  functions depending on the image extension.
// TODO - this could be neater
fn get_image(path : &Path) -> (gtk::Image, usize, usize, f32, f32) {
    let dummy : gtk::Image = gtk::Image::new();
    if path.extension().unwrap() == "fits" {
        let (image, width, height, mini, maxi) = get_image_fits(path);
        return (image, width, height, mini, maxi);
    } else if path.extension().unwrap() == "tif" ||
        path.extension().unwrap() == "tiff" {
        let (image, width, height, mini, maxi) = get_image_tiff(path);
        return (image, width, height, mini, maxi);
    }
    (dummy, 0, 0, 0.0, 0.0)
}

// Our Explorer struct/class implementation. Mostly just runs the GTK
// and keeps a hold on our models.
impl Explorer {
    pub fn new(image_paths : Vec<PathBuf>) -> Rc<Self> {
        let app = Application::new(
            Some("com.github.gtk-rs.examples.basic"),
            Default::default(),
        ).expect("failed to initialize GTK application");

        let image_index : Cell<usize> = Cell::new(0);

        // Base buffer
        let height : usize = 128;
        let width : usize = 128;
        let mut tbuf : Vec<Vec<f32>> = vec![];
        for _y in 0..height {
            let mut row  : Vec<f32> = vec![];
            for _x in 0..width {
                row.push(0 as f32);
            }
            tbuf.push(row);
        }

        let explorer = Rc::new(Self {
            app,
            image_paths,
            image_index,
        });

        explorer
    }

    // Meat of the program
    pub fn run(&self, app: Rc<Self>) {
        let app = app.clone();
        let _args: Vec<String> = env::args().collect();
 
        self.app.connect_activate( move |gtkapp| {
            let window = ApplicationWindow::new(gtkapp);
            let mut title: String = "FEX: ".to_owned();
            let opath: String = app.image_paths[0].to_str().unwrap().to_string();
            title.push_str(&opath);
            window.set_title(&title);
            window.set_default_size(350, 350);
            let vbox = gtk::Box::new(gtk::Orientation::Vertical, 3);
            let dbox = gtk::Box::new(gtk::Orientation::Vertical, 3);
            let ibox = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 3);
            let (image, width, height, mini, maxi) = get_image(&(app.image_paths[0]));
            ibox.add(&image);
            let dimstr = format!("width/height: {}x{}", width, height); 
            let label = Label::new(Some(&dimstr));
            dbox.add(&label);
            let dimstr = format!("min/max: {}x{}", mini, maxi); 
            let label2 = Label::new(Some(&dimstr));
            dbox.add(&label2);
            ibox.add(&dbox);
            vbox.add(&ibox);
            vbox.add(&hbox);
            window.add(&vbox);

            // Now look at buttons
            let button_accept = Button::new_with_label("Next");
            let ibox_arc = Arc::new(Mutex::new(ibox));
            let ibox_accept = ibox_arc.clone();
            let app_accept = app.clone();
            let win_accept = window.clone();

            // Accept button
            button_accept.connect_clicked( move |_button| {
                //println!("Accepted {}", app_accept.image_index.get());
                let mi = app_accept.image_index.get();
                if mi + 1 >= app_accept.image_paths.len() {
                    println!("All images checked! Starting again.");
                    app_accept.image_index.set(0);
                } else {
                    app_accept.image_index.set(mi + 1);
                }
            
                // Now move on to the next image
                let ibox_ref = ibox_accept.lock().unwrap();
                let children : Vec<gtk::Widget> = (*ibox_ref).get_children();
                let (image, width, height, mini, maxi) = get_image(&(app_accept.image_paths[mi + 1]));
                for i in 0..children.len() {
                    (*ibox_ref).remove(&children[i]);
                }

                let dbox = gtk::Box::new(gtk::Orientation::Vertical, 3);
                let dimstr = format!("width/height: {}x{}", width, height); 
                let label = Label::new(Some(&dimstr));
                dbox.add(&label);
                let dimstr = format!("min/max: {}x{}", mini, maxi); 
                let label2 = Label::new(Some(&dimstr));
                dbox.add(&label2);
                (*ibox_ref).add(&image);
                (*ibox_ref).add(&dbox);
                let mut title: String = "FEX: ".to_owned();
                let opath: String = app_accept.image_paths[0].to_str().unwrap().to_string();
                title.push_str(&opath);
                win_accept.set_title(&title);
                win_accept.show_all();
            });

            hbox.add(&button_accept);
            window.show_all()

        });

        self.app.run(&[]);
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();

    let mut image_files : Vec<PathBuf> = vec!();
    
    if args.len() < 2 {
        println!("Usage: explorer <path to directory of tiff / fits files>"); 
        process::exit(1);
    }

    let paths = fs::read_dir(Path::new(&args[1])).unwrap();

    for path in paths {
        match path {
            Ok(file) => {
                let filename = file.file_name();
                let tx = filename.to_str().unwrap();
                if tx.contains("tif") || tx.contains("fits") {
                    println!("Found tiff / fits: {}", tx);

                    let mut owned_string: String = args[1].to_owned();
                    let borrowed_string: &str = "/";
                    owned_string.push_str(borrowed_string);
                    owned_string.push_str(&tx.to_string());
                    image_files.push(PathBuf::from(owned_string));
                }
            },
            Err(_) => {
                println!("Error walking directory.");
            }
            
        }
       
    }
    if image_files.len() > 0 {
        image_files.sort_unstable();
        gtk::init().expect("Unable to start GTK3");
        let app = Explorer::new(image_files);
        app.run(app.clone());
    } else {
        println!("No image files found in {}.", &args[1]);
    }
}