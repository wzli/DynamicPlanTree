extends GraphNode

var plan

onready var box = get_child(0).get_child(0)
onready var name_edit = box.get_node("NameEdit")
onready var behaviour = box.get_node("BehaviourOption")
onready var interval = box.get_node("IntervalSpinBox")
onready var active = box.get_node("ActiveButton")


func update_plan(update):
	plan = update
	name_edit.text = plan["name"]
	for behaviour_name in plan["behaviour"]:
		behaviour.set_behaviour(behaviour_name)
	interval.value = plan["run_interval"]
	active.toggle_mode = plan["active"]
