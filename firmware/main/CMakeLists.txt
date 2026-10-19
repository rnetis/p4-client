idf_component_register(SRCS "main.c"
                       INCLUDE_DIRS ".")

# Components under components/ implement the HAL (board, display, touch,
# usb, wifi, power). They are added here as they are brought up on the
# real board (PRD Phase 0 / Rule 7).
