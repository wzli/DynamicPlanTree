extends AcceptDialog

func _on_error(msg):
	dialog_text = msg
	popup_centered()
