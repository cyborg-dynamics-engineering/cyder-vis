extends SceneTree


func _initialize() -> void:
	_run.call_deferred()


func _reply(decoder: Mf4Decoder) -> Dictionary:
	for attempt in 500:
		var reply := decoder.poll()
		if not reply.is_empty():
			var result: Dictionary = JSON.parse_string(reply)
			assert(result.kind != "error", str(result.get("error", "")))
			return result
		await create_timer(0.005).timeout
	assert(false, "MF4 decoder timed out")
	return {}


func _run() -> void:
	Engine.max_fps = 120
	var bridge := GodotCanBridge.new()
	var mode := OS.get_environment("CYDERVIS_TIMESTAMP_EPOCH").to_int()
	var enabled := OS.get_environment("CYDERVIS_TIMESTAMP_ENABLED") == "1"
	var supported := OS.get_environment("CYDERVIS_TIMESTAMP_SUPPORTED") == "1"
	assert(bridge.set_timestamp_epoch(mode).is_empty())
	var app_ticks := Time.get_ticks_usec()
	root.add_child(bridge)
	assert(bridge.load_dbc_file(ProjectSettings.globalize_path("res://tests/transmit.dbc")))
	assert(bridge.configure_bus(OS.get_environment("CYDERVIS_TIMESTAMP_INTERFACE")))
	for attempt in 500:
		if bridge.get_bitrate() == 500000:
			break
		await create_timer(0.005).timeout
	assert(bridge.get_bitrate() == 500000)
	assert(bridge.get_hardware_timestamp_status() == ((2 if enabled else 1) if supported else 0))
	# Exercise recording started after the app's receive timeline is underway.
	await create_timer(0.25).timeout
	var origin := float(Time.get_ticks_usec() - app_ticks) * 1e-6
	assert(bridge.start_recording(OS.get_environment("CYDERVIS_TIMESTAMP_DIRECTORY"), 2, "mf4").is_empty())
	assert(not bridge.set_timestamp_epoch((mode + 1) % 3).is_empty())
	assert(bridge.get_timestamp_epoch() == mode)
	var paths := bridge.recording_files().split("\n")
	var ready := FileAccess.open(OS.get_environment("CYDERVIS_TIMESTAMP_READY"), FileAccess.WRITE)
	ready.store_string("ready")
	ready.close()
	var trace := ""
	for attempt in 500:
		trace = bridge.get_recent_can_msgs()
		if trace.count("00010302") == 6:
			break
		await create_timer(0.005).timeout
	assert(bridge.get_hardware_timestamp_status() == ((3 if enabled else 1) if supported else 0))
	ready = FileAccess.open(OS.get_environment("CYDERVIS_TIMESTAMP_READY"), FileAccess.WRITE)
	ready.store_string("hardware")
	ready.close()
	for attempt in 500:
		trace = bridge.get_recent_can_msgs()
		if trace.contains("456"):
			break
		await create_timer(0.005).timeout
	assert(bridge.get_hardware_timestamp_status() == ((4 if enabled else 1) if supported else 0))
	var times: Array[float] = []
	for line in trace.split("\n"):
		if line.contains("00010302"):
			times.append(line.get_slice(" ", 0).to_float())
	assert(times.size() == 6, trace)
	assert(trace.contains("456"), "Missing host-timestamp fallback packet")
	var offsets := [0.0, 0.001, 0.002, 0.0015, 0.003, 0.004]
	for index in times.size():
		if enabled:
			assert(abs(times[index] - times[0] - offsets[index]) < 1e-6, trace)
	match mode:
		0: assert(abs(times[0] - Time.get_unix_time_from_system()) < 5.0)
		1: assert(times[0] >= origin and times[0] < origin + 1.0)
		2: assert(times[0] >= 0.0 and times[0] < 1.0)
	assert(bridge.stop_recording().is_empty())
	var decoder := Mf4Decoder.new()
	for path: String in paths:
		assert(decoder.open_capture(path).is_empty())
		await _reply(decoder)
		assert(decoder.request_page(0, 0, 500).is_empty())
		var result := await _reply(decoder)
		var rows: Array = result.page.rows
		var captured: Array[float] = []
		for row: Array in rows:
			if path.contains("decoded") or row[2] == "10302":
				captured.append(str(row[0]).to_float())
		assert(captured.size() == 6)
		for index in captured.size():
			if enabled:
				assert(abs(captured[index] - captured[0] - offsets[index]) < 1e-6)
		assert(abs(times[0] - captured[0]) < 1e-6, "Trace and MF4 time origins disagree")
	decoder.close_capture()
	# A failed recording must not reset the trace's time origin.
	assert(not bridge.start_recording("Z:/nonexistent/cyder-recording", 0, "csv").is_empty())
	assert(bridge.get_recent_can_msgs() == trace)
	assert(bridge.set_timestamp_epoch(2).is_empty())
	await create_timer(0.03).timeout
	assert(bridge.start_recording(OS.get_environment("CYDERVIS_TIMESTAMP_DIRECTORY"), 0, "csv").is_empty())
	assert(bridge.get_recent_can_msgs().get_slice(" ", 0).to_float() < 0.0, "Recording origin did not restart")
	assert(bridge.stop_recording().is_empty())
	bridge.close_bus()
	assert(bridge.get_hardware_timestamp_status() == 0)
	bridge.queue_free()
	await process_frame
	print("Hardware timestamp trace and MF4 tests passed")
	quit()
