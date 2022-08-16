# MQTT Rústico
Proyecto de la materia de Taller de Programación I - FIUBA - 2do Cuatrimestre 2021. Implementación del protocolo MQTT 3.1.1 en una aplicación cliente con interfaz gráfica y el respectivo servidor.

> [consigna original](https://taller-1-fiuba-rust.github.io/proyecto/anteriores/21C2/proyecto.html) 

## Adición para el examen integrador
Se agregó un cliente al servidor MQTT que simula ser un termómetro que publica las temperaturas detectadas a un tópico predeterminado. De esta manera, un servidor HTTP suscrito a este tópico muestra las temperaturas detectadas por medio de una página web.

> [apendice final](https://taller-1-fiuba-rust.github.io/proyecto/anteriores/21C2/finales_2022_feb_mar.html)

## Grupo Rostov FC
* **Elián Foppiano** - [efoppiano](https://github.com/efoppiano)
* **Facundo Mastricchio** - [FacuMastri](https://github.com/FacuMastri)
* **Nicolas Zulaica** - [Nicolas Zulaica](https://github.com/NicolasEzequielZulaicaRivera)
* **Felipe de Luca** - [fdelu](https://github.com/fdelu)

## Desarrollo

- [Cronograma](https://docs.google.com/spreadsheets/d/1GwXE0hzUsEQD60ySIDfA1ykUeKGzx8qoUHdrjYiS6MI/edit#gid=0) : 
Progreso temporal de las etapas de desarrollo
- [Diagramas](https://lucid.app/lucidchart/b9cc7a8b-a714-40ad-854d-b28e92b47682/edit?viewport_loc=3032%2C1339%2C2228%2C1088%2C0_0&invitationId=inv_8b4f792e-af0e-4608-80dc-d48d471b345a) :
Visualizacicones del funcionamiento de ciertos segmentos remarcables de la implementacion
- [Presentacion](https://docs.google.com/presentation/d/1iLrCO9CLSJpPpWfMjqDVXsS5I3QFKEdAbuzXXpbOtpo/edit?usp=sharing) :
Diapositivas explicativas del desarrollo y el funcionamiento la implementacion

## Comandos
Algunos de los comandos disponibles en el Makefile son:
* **make test:** ejecuta las pruebas de todo el proyecto
* **make run-server:** abre el servidor HTTP y la [página web](http://localhost:3030)
* **make run-thermometer** abre el termómetro
* **make**: ejecuta el linter, clippy y las pruebas de todo el proyecto

## Servidor de prueba
Tenemos un servidor de prueba disponible abierto todo el día.
Datos de conexión:
* **IP:** 144.22.236.17
* **Puerto:** 1883
* **Usuario:** demo
* **Clave:** demo
