extends ConfirmationDialog


func _ready():
	$TextEdit.text = JSON.print(Global.plan_tree, "  ")


func _on_MenuButton_id_pressed(id):
	if id == 1:
		show()


func _confirmed():
	var parsed = JSON.parse($TextEdit.text)
	if parsed.error:
		Global.error_msg(parsed.error_string + " at line " + String(parsed.error_line))
	elif verify(parsed.result):
		Global.update_plan_tree(parsed.result)
		hide()


func verify(plan):
	if not plan.has("name"):
		Global.error_msg("Plan has no name.")
		return false
	if not plan.has("behaviour"):
		Global.error_msg(plan["name"] + " plan has no behaviour.")
		return false
	if not plan.has("transitions"):
		Global.error_msg(plan["name"] + " plan has no transitions vector.")
		return false
	if not plan.has("plans"):
		Global.error_msg(plan["name"] + " plan has no plans vector.")
		return false
	for child in plan["plans"]:
		if not verify(child):
			return false
	return true
