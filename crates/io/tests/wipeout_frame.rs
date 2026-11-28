//! The drawing's WIPEOUTVARIABLES object sets WIPEOUTFRAME: AutoCAD files with hidden wipeout
//! frames (group 70 = 0) must not show them.

fn dxf(frame: Option<i32>) -> String {
    let mut s = String::from("  0\nSECTION\n  2\nENTITIES\n  0\nENDSEC\n  0\nSECTION\n  2\nOBJECTS\n");
    if let Some(f) = frame {
        s.push_str(&format!("  0\nWIPEOUTVARIABLES\n  5\nA1\n100\nAcDbWipeoutVariables\n 70\n{f}\n"));
    }
    s.push_str("  0\nENDSEC\n  0\nEOF\n");
    s
}

#[test]
fn wipeoutvariables_sets_the_frame_flag() {
    let frame = |f| cadcraft_io::read_dxf(dxf(f).as_bytes()).unwrap().header.i64("WIPEOUTFRAME", 1);
    assert_eq!(frame(Some(0)), 0, "frames hidden");
    assert_eq!(frame(Some(1)), 1);
    assert_eq!(frame(Some(2)), 2);
    assert_eq!(frame(None), 1, "no object: the default shows frames");
    assert_eq!(frame(Some(7)), 1, "an out-of-range flag is ignored");
}

/// A drawing with `frame` set (or untouched for `None`), saved as `name` and read back.
fn saved(frame: Option<i64>, name: &str) -> cadcraft_doc::Drawing {
    cadcraft_io::dwg_worker::allow_in_process();
    let mut d = cadcraft_doc::Drawing::new_metric();
    if let Some(f) = frame {
        d.header.set_i64("WIPEOUTFRAME", f);
    }
    let bytes = cadcraft_io::write(&d, name).unwrap();
    cadcraft_io::read(&bytes, name).unwrap()
}

#[test]
fn wipeout_frame_setting_survives_a_save_in_dxf_and_dwg() {
    // The DWG bridge is unavailable on wasm and FreeBSD ARM64 (#2526); DXF still works.
    let mut names = vec!["a.dxf"];
    if cfg!(not(any(target_arch = "wasm32", all(target_os = "freebsd", target_arch = "aarch64")))) {
        names.push("a.dwg");
    }
    for name in names {
        for frame in [0, 1, 2] {
            let back = saved(Some(frame), name);
            assert_eq!(back.header.i64("WIPEOUTFRAME", 1), frame, "{name}: frame {frame}");
        }
        let untouched = saved(None, name);
        assert_eq!(untouched.header.i64("WIPEOUTFRAME", 1), 1, "{name}: default");
    }
}
