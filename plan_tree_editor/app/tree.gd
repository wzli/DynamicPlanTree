extends Tree

signal open_plan(plan)

onready var tabs := $"../../TabContainer"


func _ready():
	create_tree(create_item(), Global.plan_tree)
	$"../PlanNode".set_slot_enabled_left(0, false)
	$"../PlanNode".set_slot_enabled_right(0, false)


func create_tree(root, plan):
	root.set_text(0, plan["name"])
	for sub_plan in plan["plans"]:
		create_tree(create_item(root), sub_plan)


func _item_activated():
	var path := []
	var node := get_selected()
	while node:
		path.push_back(node.get_text(0))
		node = node.get_parent()
	var plan := Global.plan_tree
	path.pop_back()
	path.invert()
	for name in path:
		for sub_plan in plan["plans"]:
			if sub_plan["name"] == name:
				plan = sub_plan
	emit_signal("open_plan", plan)


func update_plan_tree():
	clear()
	if tabs.get_child_count():
		for tab in tabs.get_children():
			tabs.remove_child(tab)
	create_tree(create_item(), Global.plan_tree)
