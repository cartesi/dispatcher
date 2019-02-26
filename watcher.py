from __future__ import division
from asciimatics.widgets import Frame, ListBox, Layout, Divider, Text, \
    Button, TextBox, Widget
from asciimatics.scene import Scene
from asciimatics.screen import Screen
from asciimatics.exceptions import ResizeScreenError, NextScene, StopApplication
from asciimatics.event import KeyboardEvent

from asciimatics.effects import Print, Clock
from asciimatics.renderers import FigletText, Rainbow
from asciimatics.scene import Scene
from asciimatics.screen import Screen
from asciimatics.exceptions import ResizeScreenError

import sys
import sqlite3
import re

def special_match(strg, search=re.compile(r'[^ (){}\[\],]').search):
    return not bool(search(strg))

import pprint
pp = pprint.PrettyPrinter(indent=4, width=78)

import urllib2, json
url = "http://127.0.0.1:3002"
service = urllib2.urlopen(url)

def format_array(n, t, a):
    return json.dumps([format_type(n, t, k) for k in a[0:10]])

def format_type(n, t, v):
    if n == "_currentState":
        return bytearray.fromhex(v[2:]).decode().replace(u"\u0000","")
    if t == "bytes32":
        return v[0:4] + ".." + v[-4:]
    if t == "address":
        return v[0:4] + ".." + v[-4:]
    if t == "uint256":
        return int(v, 16)
    if t == "bool[]":
        return format_array(n, "bool", v)
    if t == "bytes32[]":
        return format_array(n, "bytes32", v)
    if t == "uint256[]":
        return format_array(n, "uint256", v)
    if t == "uint256[5]":
        return format_array(n, "uint256", v)
    return v

def format_instance(returned_data):
    index = format_type("index", "uint256", returned_data["index"])
    concern = returned_data["concern"]["contract_address"]
    json_data = json.loads(returned_data["json_data"])
    instance_data = {k["name"]: format_type(k["name"], k["type"], k["value"])
                     for k in json_data}
    #del instance_data["_claimer"]
    #del instance_data["_challenger"]
    sub_instances = returned_data["sub_instances"]
    sub_instances = [format_instance(k) for k in sub_instances]
    returned_dict = {#"concern": concern,
                     "index": index,
                     "instance_data": instance_data,
                     "sub_instances": sub_instances}
    if returned_dict["sub_instances"] == []:
        del returned_dict["sub_instances"]
    return returned_dict


class InstanceModel(object):
    def __init__(self):
        # Current instance when editing.
        self.current_id = None

    def get_summary(self):
        data = json.dumps("Indices")
        clen = len(data)
        f = urllib2.urlopen(url, data)
        returned_data = json.loads(f.read())
        a = [(str(i), i) for i in returned_data]
        #return a
        return [(str(i), i) for i in range(8)]

    def get_instance(self, instance_id):
        data = json.dumps({ "Instance": instance_id })
        clen = len(data)
        f = urllib2.urlopen(url, data)
        returned_data = json.loads(f.read())
        extract = format_instance(returned_data)
        s = (json.dumps(extract, indent=2, sort_keys=True)
             .replace("\\\"", "")
             .replace("\"", ""))
        s = s.split("\n")
        s = [line for line in s if not special_match(line)]
        s = "\n".join(s)
        #s = pp.pprint(extract)
        return {"notes": s}

    def get_current_instance(self):
        if self.current_id is None:
            return {"name": "", "address": "", "phone": "",
                    "email": "", "notes": ""}
        else:
            return self.get_instance(self.current_id)

instances = InstanceModel()

print(instances.get_instance(5)["notes"])

#sys.exit(0)

class ListView(Frame):
    def __init__(self, screen, model):
        super(ListView, self).__init__(screen,
                                       screen.height * 29 // 30,
                                       screen.width * 29 // 30,
                                       on_load=self._reload_list,
                                       hover_focus=True,
                                       can_scroll=False,
                                       title="Instance List")
        # Save off the model that accesses the instances database.
        self._model = model

        # Create the form for displaying the list of instances.
        self._list_view = ListBox(
            Widget.FILL_FRAME,
            model.get_summary(),
            name="instances",
            add_scroll_bar=True,
            on_change=self._on_pick,
            on_select=self._edit)
        layout = Layout([100], fill_frame=True)
        self.add_layout(layout)
        layout.add_widget(self._list_view)
        layout.add_widget(Divider())
        layout2 = Layout([1, 1, 1, 1])
        self.add_layout(layout2)
        layout2.add_widget(Button("Quit", self._quit), 3)
        self.fix()
        self._on_pick()

    def _on_pick(self):
        pass

    def _reload_list(self, new_value=None):
        self._list_view.options = self._model.get_summary()
        self._list_view.value = new_value

    def _edit(self):
        self.save()
        self._model.current_id = self.data["instances"]
        raise NextScene("Edit Instance")

    @staticmethod
    def _quit():
        raise StopApplication("User pressed quit")


class InstanceView(Frame):
    def __init__(self, screen, model):
        super(InstanceView, self).__init__(screen,
                                           screen.height * 29 // 30,
                                           screen.width * 29 // 30,
                                           hover_focus=True,
                                           can_scroll=False,
                                           title="Instance Details",
                                           reduce_cpu=True)
        # Save off the model that accesses the instances database.
        self._model = model

        # Create the form for displaying the list of instances.
        layout2 = Layout([1, 1, 1, 1])
        self.add_layout(layout2)
        button = Button("Refresh", self.reset)
        layout2.add_widget(button, 0)
        button = Button("OK", self._ok)
        layout2.add_widget(button, 3)

        layout = Layout([100], fill_frame=True)
        self.add_layout(layout)
        layout.add_widget(TextBox(
            Widget.FILL_FRAME, ">", "notes", as_string=True,
            line_wrap=True))
        self.fix()

    def reset(self):
        # Do standard reset to clear out form, then populate with new data.
        super(InstanceView, self).reset()
        self.data = self._model.get_current_instance()

    def _ok(self):
        self.save()
        raise NextScene("Main")

    @staticmethod
    def _cancel():
        raise NextScene("Main")


def demo(screen, scene):
    scenes = [
        Scene([ListView(screen, instances)], -1, name="Main"),
        Scene([InstanceView(screen, instances)], 0, name="Edit Instance")
    ]
    screen.play(scenes, stop_on_resize=True, start_scene=scene)

last_scene = None
while True:
    try:
        Screen.wrapper(demo, catch_interrupt=True, arguments=[last_scene])
        sys.exit(0)
    except ResizeScreenError as e:
        last_scene = e.scene
