# MQTT Rústico
Proyecto de la materia de Taller de Programación I - FIUBA - 2do Cuatrimestre 2021. Implementación del protocolo MQTT 3.1.1 en una aplicación cliente con interfaz gráfica y el respectivo servidor.

## Grupo Rostov FC
* **Elián Foppiano** - [efoppiano](https://github.com/efoppiano)
* **Facundo Mastricchio** - [FacuMastri](https://github.com/FacuMastri)
* **Nicolas Zulaica** - [Nicolas Zulaica](https://github.com/NicolasEzequielZulaicaRivera)
* **Felipe de Luca** - [fdelu](https://github.com/fdelu)

## Diagramas
[Link](
https://lucid.app/lucidchart/b9cc7a8b-a714-40ad-854d-b28e92b47682/edit?viewport_loc=3032%2C1339%2C2228%2C1088%2C0_0&invitationId=inv_8b4f792e-af0e-4608-80dc-d48d471b345a)

## Cronograma
[Link](
https://docs.google.com/spreadsheets/d/1GwXE0hzUsEQD60ySIDfA1ykUeKGzx8qoUHdrjYiS6MI/edit#gid=0)

## Comandos
Algunos de los comandos disponibles en el Makefile son:
* **make test:** ejecuta las pruebas de todo el proyecto
* **make run-server:** abre el servidor
* **make run-client:** abre el cliente
* **make run:** abre el cliente y el servidor
* **make stress:** abre el servidor y ejecuta una [prueba de estrés](https://github.com/inovex/mqtt-stresser) sobre este *[requiere [docker](https://www.docker.com/)]*
* **make**: ejecuta el linter, clippy y las pruebas de todo el proyecto

## Servidor de prueba
Tenemos un servidor de prueba disponible abierto todo el día.
Datos de conexión:
* **IP:** 150.230.76.67
* **Puerto:** 1883
* **Usuario:** demo
* **Clave:** demo
