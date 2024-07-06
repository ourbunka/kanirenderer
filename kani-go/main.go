package main

/*
#cgo LDFLAGS: -L./lib/ -lkanirenderer_viewer
#include "./lib/kanirenderer_viewer.h"
#include <stdlib.h>
*/
import "C"
import (
	"flag"
	"log"
	"runtime"
)

// fix panicked at 'Initializing the event loop outside of the main thread by
// locking kanirenderer to main thread, which required by winit
func init() {
	runtime.LockOSThread()
}

// using kanirenderer in go
func main() {
	log.Println("kanirenderer in go")
	var path string
	flag.StringVar(&path, "path", "", "enter file path ")
	var filet string
	flag.StringVar(&filet, "type", "opengl", "enter file type")
	var mode string
	flag.StringVar(&mode, "mode", "fullscreen", "enter window mode")
	flag.Parse()
	if path == "" {
		log.Panicln("no files path provided, please provide -path=/path/to/yourobj")
	}
	filePath := C.CString(path)
	fileType := C.CString(filet)
	fullScreen := C.CString(mode)

	C.run_kanirenderer(filePath, fileType, fullScreen)
	//keep main() running
	select {}
}

//build kanirenderer dll, then copy to ./lib
//then use cbindgen to generate header file
//cbindgen --config template.toml --crate kanirenderer_viewer --output kanirenderer_viewer.h
//then copy header file to ./lib
//
//then compile with zig c compiler
// run  >> CC="zig cc -target x86_64-windows-gnu" CXX="zig c++ -target x86_64-windows-gnu" GOOS="windows" GOARCH="amd64" CGO_ENABLED=1 go build .
//then run kani-go -path=/path/to/your.obj
