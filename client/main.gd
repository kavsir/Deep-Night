extends Node

const ServerPacket = preload("res://server_packet.gd")
const World = preload("res://world.gd")
const INPUT_REPEAT_MS: int = 100
const HANDSHAKE_TIMEOUT_MS: int = 10_000

var _socket: WebSocketPeer
var _world: World
var _join_sent: bool = false
var _join_name: String
var _handshake_deadline_ms: int
var _server_error: String
var _seq: int
var _last_direction: Vector2i
var _next_repeat_ms: int
var _has_focus: bool = true

var _connection_screen: CenterContainer
var _ip_field: LineEdit
var _port_field: LineEdit
var _name_field: LineEdit
var _connect_button: Button
var _status: Label
var _notice: Label


func _ready() -> void:
	var overlay: CanvasLayer = CanvasLayer.new()
	add_child(overlay)

	_connection_screen = CenterContainer.new()
	_connection_screen.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	overlay.add_child(_connection_screen)
	var form: VBoxContainer = VBoxContainer.new()
	form.custom_minimum_size.x = 320
	form.add_theme_constant_override("separation", 8)
	_connection_screen.add_child(form)

	var title: Label = Label.new()
	title.text = "Deep Night"
	title.add_theme_font_size_override("font_size", 34)
	form.add_child(title)
	_ip_field = _add_field(form, "IP", "127.0.0.1")
	_port_field = _add_field(form, "Cổng", "7777")
	_name_field = _add_field(form, "Tên nhân vật", "")
	_name_field.text_submitted.connect(func(_value: String) -> void: _connect_to_server())

	_connect_button = Button.new()
	_connect_button.text = "Kết nối"
	_connect_button.pressed.connect(_connect_to_server)
	form.add_child(_connect_button)
	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.custom_minimum_size.y = 52
	_status.text = "Nhập tên để tham gia."
	form.add_child(_status)
	_name_field.grab_focus()

	_notice = Label.new()
	_notice.position = Vector2(16, 12)
	_notice.add_theme_color_override("font_outline_color", Color("#16201b"))
	_notice.add_theme_constant_override("outline_size", 6)
	_notice.hide()
	overlay.add_child(_notice)


func _add_field(form: VBoxContainer, caption: String, value: String) -> LineEdit:
	var label: Label = Label.new()
	label.text = caption
	form.add_child(label)
	var field: LineEdit = LineEdit.new()
	field.text = value
	form.add_child(field)
	return field


func _connect_to_server() -> void:
	if _socket != null:
		return

	var ip: String = _ip_field.text.strip_edges()
	var port_text: String = _port_field.text.strip_edges()
	var port: int = port_text.to_int()
	var player_name: String = _name_field.text.strip_edges()
	if not ip.is_valid_ip_address():
		_status.text = "Địa chỉ IP không hợp lệ."
		return
	if not port_text.is_valid_int() or port < 1 or port > 65_535:
		_status.text = "Cổng phải là số nguyên từ 1 đến 65535."
		return
	if player_name.is_empty() or player_name.length() > 20:
		_status.text = "Tên phải dài từ 1 đến 20 ký tự."
		return
	for index: int in range(player_name.length()):
		var code: int = player_name.unicode_at(index)
		if code < 32 or (code >= 127 and code <= 159):
			_status.text = "Tên không được chứa ký tự điều khiển."
			return

	var host: String = "[%s]" % ip if ip.contains(":") else ip
	var url: String = "ws://%s:%d" % [host, port]
	_socket = WebSocketPeer.new()
	var result: Error = _socket.connect_to_url(url)
	if result != OK:
		_end_session("Không thể kết nối (%s)." % error_string(result))
		return

	_join_name = player_name
	_join_sent = false
	_server_error = ""
	_handshake_deadline_ms = Time.get_ticks_msec() + HANDSHAKE_TIMEOUT_MS
	_connect_button.disabled = true
	_status.text = "Đang kết nối tới %s…" % url


func _process(_delta: float) -> void:
	if _socket == null:
		return

	_socket.poll()
	var state: WebSocketPeer.State = _socket.get_ready_state()
	if state == WebSocketPeer.STATE_OPEN and not _join_sent:
		if not _send({"type": "Join", "name": _join_name}):
			return
		_join_sent = true
		_status.text = "Đã kết nối. Đang chờ vào bản đồ…"

	while _socket.get_available_packet_count() > 0:
		var bytes: PackedByteArray = _socket.get_packet()
		if _socket.get_packet_error() != OK or not _socket.was_string_packet():
			push_warning("Bỏ qua khung WebSocket không phải văn bản hợp lệ.")
			continue
		var packet: Dictionary = ServerPacket.decode(bytes.get_string_from_utf8())
		if not packet.is_empty():
			_handle_packet(packet)

	if state == WebSocketPeer.STATE_CLOSED:
		var code: int = _socket.get_close_code()
		var reason: String = _socket.get_close_reason()
		if code != -1:
			var close_message: String = "Máy chủ đã đóng kết nối."
			if _world == null and _join_sent:
				close_message = "Máy chủ đã đóng kết nối trước khi vào bản đồ."
			if code >= 1000 and code != 1005:
				close_message += " Mã đóng: %d." % code
			if not reason.is_empty():
				close_message += " " + reason
			_end_session(close_message)
		elif _world != null:
			_end_session("Đã mất kết nối với máy chủ.")
		elif _join_sent:
			_end_session("Đã mất kết nối trước khi vào bản đồ.")
		else:
			_end_session("Không thể mở kết nối tới máy chủ.")
	elif _world == null and Time.get_ticks_msec() >= _handshake_deadline_ms:
		if _join_sent:
			_end_session("Hết thời gian chờ vào bản đồ.")
		else:
			_end_session("Không thể mở kết nối trong thời gian chờ.")
	elif state == WebSocketPeer.STATE_OPEN and _world != null:
		_send_movement()


func _handle_packet(packet: Dictionary) -> void:
	var kind: String = packet["type"]
	if kind == "Error":
		_server_error = packet["message"]
		var message: String = "Lỗi từ máy chủ: %s" % _server_error
		if _world == null:
			_status.text = message
		else:
			_notice.text = message
			_notice.show()
		return

	if kind == "Welcome":
		if not _join_sent or _world != null:
			push_warning("Bỏ qua Welcome ngoài giai đoạn chờ vào bản đồ.")
			return
		_world = World.new()
		_world.local_id = int(packet["player_id"])
		_world.tick_rate = int(packet["tick_rate"])
		add_child(_world)
		_world.add_player(_world.local_id, _join_name)
		_seq = 0
		_last_direction = Vector2i.ZERO
		_next_repeat_ms = 0
		_server_error = ""
		_connection_screen.hide()
		return

	if _world == null:
		push_warning("Bỏ qua %s trước Welcome." % kind)
		return

	match kind:
		"PlayerJoined":
			_world.add_player(int(packet["id"]), packet["name"])
		"PlayerLeft":
			_world.remove_player(int(packet["id"]))
		"Snapshot":
			_world.apply_snapshot(
				packet["tick"],
				packet["positions"],
				packet.get("monsters", []),
				packet.get("npcs", [])
			)


func _notification(what: int) -> void:
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT:
		_has_focus = false
	elif what == NOTIFICATION_APPLICATION_FOCUS_IN:
		_has_focus = true


func _send_movement() -> void:
	var direction: Vector2i = Vector2i.ZERO
	if _has_focus:
		direction.x = (
			int(Input.is_action_pressed("move_right")) - int(Input.is_action_pressed("move_left"))
		)
		direction.y = (
			int(Input.is_action_pressed("move_down")) - int(Input.is_action_pressed("move_up"))
		)

	var now_ms: int = Time.get_ticks_msec()
	var direction_changed: bool = direction != _last_direction
	var repeat_due: bool = direction != Vector2i.ZERO and now_ms >= _next_repeat_ms
	if not direction_changed and not repeat_due:
		return

	_seq += 1
	if _send({"type": "Input", "seq": _seq, "dx": direction.x, "dy": direction.y}):
		_last_direction = direction
		_next_repeat_ms = now_ms + INPUT_REPEAT_MS


func _send(packet: Dictionary) -> bool:
	var result: Error = _socket.send_text(JSON.stringify(packet))
	if result != OK:
		_end_session("Không thể gửi gói %s (%s)." % [packet["type"], error_string(result)])
		return false
	return true


func _end_session(reason: String) -> void:
	if _socket != null:
		_socket.close(-1)
		_socket = null
	if _world != null:
		remove_child(_world)
		_world.queue_free()
		_world = null
	_notice.hide()
	_connection_screen.show()
	_connect_button.disabled = false
	_status.text = reason.strip_edges()
	if not _server_error.is_empty():
		_status.text += "\nLỗi từ máy chủ: %s" % _server_error
	_server_error = ""
	_name_field.grab_focus()
