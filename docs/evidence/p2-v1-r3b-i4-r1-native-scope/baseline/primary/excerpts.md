# B-R1-7 and section 17: primary-source excerpts, verbatim

Retrieved documents, with time and SHA-256: `retrieval.txt`. Text conversions (tags stripped, whitespace collapsed, no word changed): the `.txt` files beside them. Read-only research: no bus was contacted and no systemd tool was run.

## D-Bus: method calls and replies (the D-Bus Specification)

> dbus-specification.txt, lines 1552-1555:
>  When an application handles a method call message, it is required to
>  return a reply. The reply is identified by a REPLY_SERIAL header field
>  indicating the serial number of the METHOD_CALL being replied to. The
>  reply can have one of two types; either METHOD_RETURN or ERROR.

> dbus-specification.txt, lines 1563-1565:
>  Even if a method call has no return values, a METHOD_RETURN
>  reply is required, so the caller will know the method
>  was successfully processed.

> dbus-specification.txt, lines 417-419:
>  this is an interesting optimization. D-Bus is also designed to
>  avoid round trips and allow asynchronous operation, much like
>  the X protocol.

The specification matches a reply to its call by REPLY_SERIAL and is designed for asynchronous operation. It contains no statement that a recipient processes or answers concurrently issued calls in the order they were sent (searched for: order, ordering, reorder, sequence, FIFO, guarantee, deliver, queue; see the conversion).

## D-Bus: the delivery-order rule as stated by the NetworkManager developers ("Notes on D-Bus")

> networkmanager-notes-on-dbus.txt, lines 136-139:
> It’s an important feature that the order of messages is preserved, at least for
> messages between the two same peers. The only exception is that a response to
> a method call might overtake a response to an earlier call when the callee side
> chooses to answer the latter request first.

## systemd (the host's installed manual, systemd 255.4-1ubuntu8.17)

> org.freedesktop.systemd1.v255-host.txt, lines 786-790:
>        StartTransientUnit() may be used to create and start a transient unit which will be released as soon
>        as it is not running or referenced anymore or the system is rebooted.  name is the unit name
>        including its suffix and must be unique.	 mode is the same as in StartUnit(), properties contains
>        properties of the unit, specified like in SetUnitProperties().  aux is currently unused and should
>        be passed as an empty array. See the New Control Group Interface[2] for more information how to make

> org.freedesktop.systemd1.v255-host.txt, lines 536-537:
>        GetUnit() may be used to get the unit object path for a unit name. It takes the unit name and
>        returns the object path. If a unit has not been loaded yet by this name this method will fail.

> org.freedesktop.systemd1.v255-host.txt, lines 1226-1226:
>        Id contains the primary name of the unit.

> org.freedesktop.systemd1.v255-host.txt, lines 4376-4378:
> SCOPE UNIT OBJECTS
>        All scope unit objects implement the org.freedesktop.systemd1.Scope interface (described here) in
>        addition to the generic org.freedesktop.systemd1.Unit interface (see above).

> org.freedesktop.systemd1.v255-host.txt, lines 4398-4401:
> 		 @org.freedesktop.DBus.Property.EmitsChangedSignal("const")
> 		 readonly s OOMPolicy = '...';
> 		 @org.freedesktop.DBus.Property.EmitsChangedSignal("false")
> 		 readonly s Slice = '...';

> org.freedesktop.systemd1.v255-host.txt, lines 2121-2121:
>        ControlGroup indicates the control group path the processes of this service unit are placed in.

## systemd (online, latest)

> org.freedesktop.systemd1.txt, lines 962-966:
> StartTransientUnit() may be used to create and start a transient unit which
>  will be released as soon as it is not running or referenced anymore or the system is
>  rebooted. name is the unit name including its suffix and must be
>  unique. mode is the same as in StartUnit(),
>  properties contains properties of the unit, specified like in

> org.freedesktop.systemd1.txt, lines 602-604:
> GetUnit() may be used to get the unit object path for a unit name. It takes
>  the unit name and returns the object path. If a unit has not been loaded yet by this name this method
>  will fail.

> org.freedesktop.systemd1.txt, lines 1533-1533:
> Id contains the primary name of the unit.

Scope objects carry `ControlGroup` (`readonly s`) on `org.freedesktop.systemd1.Scope` and `Id` on `org.freedesktop.systemd1.Unit`. `ControlGroupId` (`readonly t`) is listed but not described in either manual; I4-R1 does not use it.
