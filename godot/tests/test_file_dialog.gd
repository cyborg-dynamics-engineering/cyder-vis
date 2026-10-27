extends SceneTree


func _initialize() -> void:
	var key := "USERPROFILE" if OS.get_name() == "Windows" else "HOME"
	var original_home := OS.get_environment(key)
	var test_home := ProjectSettings.globalize_path("user://test home")
	DirAccess.make_dir_recursive_absolute(test_home)
	OS.set_environment(key, test_home)
	var button := DbcFileButton.new()
	var dialog := button.create_file_dialog()
	assert(dialog.access == FileDialog.ACCESS_FILESYSTEM)
	assert(dialog.file_mode == FileDialog.FILE_MODE_OPEN_FILE)
	assert(dialog.current_dir.replace("\\", "/").trim_suffix("/") == DbcFileButton.home_directory().replace("\\", "/").trim_suffix("/"))
	dialog.free()
	button.free()
	OS.set_environment(key, original_home)
	DirAccess.remove_absolute(test_home)
	print("DBC file dialog tests passed")
	quit()
